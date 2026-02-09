use std::net::SocketAddr;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures::{stream::StreamExt, SinkExt};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    protocol::{Incoming, Outgoing, UserInfo},
    state::{AppState, Client},
    utils::now_ms,
};

fn trust_proxy_headers_configured() -> bool {
    std::env::var("TRUST_PROXY_HEADERS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

fn should_trust_proxy_headers(addr: SocketAddr) -> bool {
    // Bun reverse proxy on the same host appears as loopback; trust its forwarded headers.
    // Non-loopback peers must opt in explicitly via TRUST_PROXY_HEADERS=true.
    addr.ip().is_loopback() || trust_proxy_headers_configured()
}

fn extract_client_ip(headers: &HeaderMap, addr: SocketAddr, trust_proxy: bool) -> String {
    if !trust_proxy {
        return addr.ip().to_string();
    }

    // Only use forwarding headers when explicitly enabled (trusted reverse proxy setup).
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next()) // Take first IP if multiple
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| addr.ip().to_string())
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, addr, should_trust_proxy_headers(addr));

    ws.on_upgrade(move |socket| handle_socket(state, socket, client_ip))
}

/// Buffer size for outbound messages per client.
/// If a client can't keep up, messages will be dropped to prevent memory exhaustion.
const CLIENT_CHANNEL_BUFFER: usize = 256;

async fn handle_socket(state: AppState, socket: WebSocket, client_ip: String) {
    let id = Uuid::new_v4();
    let name = format!("guest-{}", &id.to_string()[..6]);
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(CLIENT_CHANNEL_BUFFER);

    // Send loop
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(err) = sender.send(msg).await {
                error!(?err, "WS send loop stopped");
                break;
            }
        }
        debug!("WS send loop finished");
    });

    let client = Client::new(name.clone(), client_ip.clone(), tx);

    // Register client and update stats
    state.clients.insert(id, client.clone());
    state.increment_connections();

    info!(id = %id, name = %name, ip = %client_ip, "Client connected");

    // Send welcome messages
    client.send(&Outgoing::AckName {
        name: name.clone(),
        at: now_ms(),
    });
    broadcast(
        &state,
        &Outgoing::System {
            text: format!("{name} heeft de chat betreden."),
            at: now_ms(),
        },
        Some(id),
    );

    // Receive loop
    while let Some(msg) = receiver.next().await {
        debug!(id = %id, raw = ?msg, "Ontvangen WS bericht");
        let msg = match msg {
            Ok(m) => m,
            Err(err) => {
                debug!(id = %id, ?err, "WS receive error (client disconnected abruptly)");
                break;
            }
        };
        match msg {
            Message::Text(text) => {
                if let Err(err) = process_message(&state, id, text.to_string()).await {
                    if let Some(entry) = state.clients.get(&id) {
                        entry.value().send(&Outgoing::Error { message: err });
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(p) => {
                let _ = client.tx.try_send(Message::Pong(p));
            }
            _ => {}
        }
    }

    // Cleanup: get name BEFORE removing
    let final_name = state
        .clients
        .get(&id)
        .map(|entry| entry.value().name.clone())
        .unwrap_or_else(|| name.clone());

    state.clients.remove(&id);

    broadcast(
        &state,
        &Outgoing::System {
            text: format!("{final_name} heeft de chat verlaten."),
            at: now_ms(),
        },
        Some(id),
    );

    send_task.abort();
    info!(id = %id, name = %final_name, ip = %client_ip, "Client disconnected");
}

async fn process_message(state: &AppState, id: Uuid, text: String) -> Result<(), String> {
    let incoming: Incoming =
        serde_json::from_str(&text).map_err(|_| "Bericht moet geldig JSON zijn.".to_string())?;

    match incoming {
        Incoming::Chat { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err("Message cannot be empty.".into());
            }
            if trimmed.len() > 500 {
                return Err("Message is too long (max 500 characters).".into());
            }

            // Check rate limit
            let (name, ip) = {
                let entry = state
                    .clients
                    .get(&id)
                    .ok_or_else(|| "Unknown user".to_string())?;

                // Check rate limit before allowing the message
                if let Err(wait_secs) = entry.value().check_rate_limit(&state.rate_limit) {
                    return Err(format!(
                        "Rate limit exceeded. Please wait {} seconds.",
                        wait_secs
                    ));
                }

                (entry.value().name.clone(), entry.value().ip.clone())
            };

            state.increment_messages();
            broadcast(
                state,
                &Outgoing::Chat {
                    from: name.clone(),
                    text: trimmed.to_string(),
                    at: now_ms(),
                },
                None,
            );
            debug!(from = %name, id = %id, ip = %ip, "Bericht verzonden");
        }
        Incoming::SetName { name } => {
            let trimmed = name.trim();
            if trimmed.len() < 2 || trimmed.len() > 32 {
                return Err("Naam moet tussen 2 en 32 tekens zijn.".into());
            }
            if !trimmed
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
            {
                return Err("Naam mag alleen letters, cijfers, spaties, - en _ bevatten.".into());
            }

            let rename_info = {
                if let Some(mut entry) = state.clients.get_mut(&id) {
                    let old = entry.value().name.clone();
                    entry.name = trimmed.to_string();
                    entry.send(&Outgoing::AckName {
                        name: entry.name.clone(),
                        at: now_ms(),
                    });
                    Some((old, entry.name.clone(), entry.ip.clone()))
                } else {
                    None
                }
            };

            if let Some((old, new_name, ip)) = rename_info {
                broadcast(
                    state,
                    &Outgoing::System {
                        text: format!("{old} heet nu {new_name}."),
                        at: now_ms(),
                    },
                    Some(id),
                );
                debug!(old = %old, new = %new_name, id = %id, ip = %ip, "Gebruikersnaam gewijzigd");
            }
        }
        Incoming::Status => {
            let uptime_secs = state.uptime_seconds();
            let count = state.user_count();
            let messages = state.messages_sent();
            let msgs_per_sec = if uptime_secs > 0 {
                messages as f64 / uptime_secs as f64
            } else {
                0.0
            };
            let memory_mb = state.memory_mb().await;
            let cpu_cores = std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1);

            if let Some(entry) = state.clients.get(&id) {
                entry.value().send(&Outgoing::Status {
                    version: env!("CARGO_PKG_VERSION"),
                    rust_version: env!("RUSTC_VERSION"),
                    os: std::env::consts::OS,
                    cpu_cores,
                    uptime_seconds: uptime_secs,
                    user_count: count,
                    peak_users: state.peak_users() as usize,
                    connections_total: state.connections_total(),
                    messages_sent: messages,
                    messages_per_second: (msgs_per_sec * 100.0).round() / 100.0,
                    memory_mb: (memory_mb * 100.0).round() / 100.0,
                    ai_enabled: state.ai.is_enabled(),
                    ai_model: if state.ai.is_enabled() {
                        Some(state.ai.model().to_string())
                    } else {
                        None
                    },
                });
            }
        }
        Incoming::ListUsers => {
            let users: Vec<UserInfo> = state.list_users();
            if let Some(entry) = state.clients.get(&id) {
                entry.value().send(&Outgoing::ListUsers { users });
            }
        }
        Incoming::Ping { token } => {
            if let Some(entry) = state.clients.get(&id) {
                entry.value().send(&Outgoing::Pong {
                    token,
                    at: now_ms(),
                });
            }
        }
        Incoming::Ai { prompt } => {
            let (name, rate_limit_key) = state
                .clients
                .get(&id)
                .map(|e| (e.value().name.clone(), e.value().ip.clone()))
                .unwrap_or_else(|| ("unknown".to_string(), id.to_string()));

            // Query AI (this may take a few seconds)
            match state.ai.query(&rate_limit_key, &prompt).await {
                Ok(ai_response) => {
                    broadcast(
                        state,
                        &Outgoing::Ai {
                            from: name.clone(),
                            prompt: prompt.clone(),
                            response: ai_response.content,
                            response_ms: ai_response.response_ms,
                            tokens: ai_response.tokens,
                            cost: ai_response.cost,
                            at: now_ms(),
                        },
                        None,
                    );
                    debug!(from = %name, id = %id, prompt_len = prompt.len(), "AI response sent");
                }
                Err(err) => {
                    if let Some(entry) = state.clients.get(&id) {
                        entry.value().send(&Outgoing::Error { message: err });
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn broadcast(state: &AppState, payload: &Outgoing, except: Option<Uuid>) {
    let text = serde_json::to_string(payload)
        .unwrap_or_else(|_| r#"{"type":"error","message":"serialize"}"#.into());

    let targets = state.clients.len();
    debug!(targets, except = ?except, kind = %payload.kind(), "Broadcast payload");

    for entry in state.clients.iter() {
        if except.is_some_and(|ex| ex == *entry.key()) {
            continue;
        }
        // Use try_send to avoid blocking on slow clients
        if let Err(err) = entry
            .value()
            .tx
            .try_send(Message::Text(text.clone().into()))
        {
            error!(id = %entry.key(), ?err, "Send to client failed (slow client or disconnected)");
        }
    }
}
