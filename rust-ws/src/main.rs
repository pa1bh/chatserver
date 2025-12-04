use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, atomic::AtomicU64},
    time::{Instant, SystemTime},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{stream::StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};
use uuid::Uuid;

type Clients = Arc<Mutex<HashMap<Uuid, Client>>>;

#[derive(Clone)]
struct AppState {
    clients: Clients,
    started_at: Instant,
    messages_sent: Arc<AtomicU64>,
}

#[derive(Clone)]
struct Client {
    name: String,
    ip: String,
    tx: mpsc::UnboundedSender<Message>,
    #[allow(dead_code)]
    connected_at: SystemTime,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Incoming {
    #[serde(rename = "chat")]
    Chat { text: String },
    #[serde(rename = "setName")]
    SetName { name: String },
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "listUsers")]
    ListUsers,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Outgoing {
    #[serde(rename = "chat")]
    Chat { from: String, text: String, at: u128 },
    #[serde(rename = "system")]
    System { text: String, at: u128 },
    #[serde(rename = "ackName")]
    AckName { name: String, at: u128 },
    #[serde(rename = "status")]
    Status {
        #[serde(rename = "uptimeSeconds")]
        uptime_seconds: f64,
        #[serde(rename = "userCount")]
        user_count: usize,
        #[serde(rename = "messagesSent")]
        messages_sent: u64,
    },
    #[serde(rename = "listUsers")]
    ListUsers { users: Vec<UserInfo> },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize, Clone)]
struct UserInfo {
    id: String,
    name: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let port = std::env::var("WS_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);
    let addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], port));

    let state = AppState {
        clients: Arc::new(Mutex::new(HashMap::new())),
        started_at: Instant::now(),
        messages_sent: Arc::new(AtomicU64::new(0)),
    };

    let app = Router::new()
        .route("/", get(ws_handler))
        .with_state(state.clone())
        .into_make_service_with_connect_info::<SocketAddr>();

    info!(port, "Rust WS server start");
    axum::Server::bind(&addr)
        .http1_only(true)
        .serve(app)
        .await
        .expect("start ws server");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, socket, addr))
}

async fn handle_socket(state: AppState, socket: WebSocket, addr: SocketAddr) {
    let id = Uuid::new_v4();
    let name = format!("guest-{}", &id.to_string()[..6]);
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Send loop
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(err) = sender.send(msg).await {
                error!(?err, "WS send loop stopped");
                break;
            }
        }
        info!("WS send loop finished");
    });

    let client = Client {
        name: name.clone(),
        ip: addr.ip().to_string(),
        tx,
        connected_at: SystemTime::now(),
    };

    {
        let mut clients = state.clients.lock().await;
        clients.insert(id, client.clone());
    }

    info!(id = %id, name = %name, ip = %addr.ip(), "Client connected");
    send_to_one(&client, &Outgoing::AckName { name: name.clone(), at: now_ms() });
    broadcast(
        &state,
        &Outgoing::System {
            text: format!("{name} heeft de chat betreden."),
            at: now_ms(),
        },
        Some(id),
    )
    .await;

    // Receive loop
    while let Some(msg) = receiver.next().await {
        info!(id = %id, raw = ?msg, "Ontvangen WS bericht");
        let msg = match msg {
            Ok(m) => m,
            Err(err) => {
                error!(id = %id, ?err, "WS receive error");
                break;
            }
        };
        match msg {
            Message::Text(text) => {
                if let Err(err) = process_message(&state, id, text).await {
                    if let Some(c) = state.clients.lock().await.get(&id).cloned() {
                        send_to_one(&c, &Outgoing::Error { message: err });
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(p) => {
                let _ = client.tx.send(Message::Pong(p));
            }
            _ => {}
        }
    }

    // Cleanup
    {
        let mut clients = state.clients.lock().await;
        clients.remove(&id);
    }
    let latest_name = {
        let clients = state.clients.lock().await;
        clients.get(&id).map(|c| c.name.clone()).unwrap_or_else(|| name.clone())
    };
    broadcast(
        &state,
        &Outgoing::System {
            text: format!("{} heeft de chat verlaten.", latest_name),
            at: now_ms(),
        },
        Some(id),
    )
    .await;

    send_task.abort();
    info!(id = %id, name = %name, ip = %addr.ip(), "Client disconnected");
}

async fn process_message(state: &AppState, id: Uuid, text: String) -> Result<(), String> {
    let incoming: Incoming = serde_json::from_str(&text).map_err(|_| "Bericht moet geldig JSON zijn.".to_string())?;

    match incoming {
        Incoming::Chat { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err("Bericht mag niet leeg zijn.".into());
            }
            if trimmed.len() > 500 {
                return Err("Bericht is te lang (max 500 tekens).".into());
            }
            let (name, ip) = {
                let clients = state.clients.lock().await;
                let entry = clients.get(&id).ok_or_else(|| "Onbekende gebruiker".to_string())?;
                (entry.name.clone(), entry.ip.clone())
            };

            state.messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            broadcast(
                state,
                &Outgoing::Chat {
                    from: name.clone(),
                    text: trimmed.to_string(),
                    at: now_ms(),
                },
                None,
            )
            .await;
            info!(from = %name, id = %id, ip = %ip, "Bericht verzonden");
        }
        Incoming::SetName { name } => {
            let trimmed = name.trim();
            if trimmed.len() < 2 || trimmed.len() > 32 {
                return Err("Naam moet tussen 2 en 32 tekens zijn.".into());
            }
            let rename_info = {
                let mut clients = state.clients.lock().await;
                if let Some(entry) = clients.get_mut(&id) {
                    let old = entry.name.clone();
                    entry.name = trimmed.to_string();
                    send_to_one(entry, &Outgoing::AckName { name: entry.name.clone(), at: now_ms() });
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
                )
                .await;
                info!(old = %old, new = %new_name, id = %id, ip = %ip, "Gebruikersnaam gewijzigd");
            }
        }
        Incoming::Status => {
            let uptime = state.started_at.elapsed().as_secs_f64();
            let count = state.clients.lock().await.len();
            let messages = state.messages_sent.load(std::sync::atomic::Ordering::Relaxed);
            if let Some(entry) = state.clients.lock().await.get(&id).cloned() {
                send_to_one(
                    &entry,
                    &Outgoing::Status {
                        uptime_seconds: uptime,
                        user_count: count,
                        messages_sent: messages,
                    },
                );
            }
        }
        Incoming::ListUsers => {
            let users = state
                .clients
                .lock()
                .await
                .iter()
                .map(|(id, c)| UserInfo {
                    id: id.to_string(),
                    name: c.name.clone(),
                })
                .collect::<Vec<_>>();
            if let Some(entry) = state.clients.lock().await.get(&id).cloned() {
                send_to_one(&entry, &Outgoing::ListUsers { users });
            }
        }
    }

    Ok(())
}

async fn broadcast(state: &AppState, payload: &Outgoing, except: Option<Uuid>) {
    let text = serde_json::to_string(payload).unwrap_or_else(|_| "{\"type\":\"error\",\"message\":\"serialize\"}".into());
    let clients = state.clients.lock().await;
    let targets = clients.len();
    info!(targets, except = ?except, kind = %payload_kind(payload), "Broadcast payload");
    for (id, client) in clients.iter() {
        if except.is_some() && except.unwrap() == *id {
            continue;
        }
        if let Err(err) = client.tx.send(Message::Text(text.clone())) {
            error!(id = %id, ?err, "Send to client failed");
        }
    }
}

fn send_to_one(client: &Client, payload: &Outgoing) {
    if let Ok(text) = serde_json::to_string(payload) {
        if let Err(err) = client.tx.send(Message::Text(text)) {
            error!(name = %client.name, ip = %client.ip, ?err, "Send to single client failed");
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn payload_kind(payload: &Outgoing) -> &'static str {
    match payload {
        Outgoing::Chat { .. } => "chat",
        Outgoing::System { .. } => "system",
        Outgoing::AckName { .. } => "ackName",
        Outgoing::Status { .. } => "status",
        Outgoing::ListUsers { .. } => "listUsers",
        Outgoing::Error { .. } => "error",
    }
}
