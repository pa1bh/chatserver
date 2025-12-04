use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc},
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
    messages_sent: Arc<tokio::sync::atomic::AtomicU64>,
}

#[derive(Clone)]
struct Client {
    name: String,
    ip: String,
    tx: mpsc::UnboundedSender<Message>,
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
#[serde(tag = "type")]
enum Outgoing {
    #[serde(rename = "chat")]
    Chat { from: String, text: String, at: u128 },
    #[serde(rename = "system")]
    System { text: String, at: u128 },
    #[serde(rename = "ackName")]
    AckName { name: String, at: u128 },
    #[serde(rename = "status")]
    Status {
        uptimeSeconds: f64,
        userCount: usize,
        messagesSent: u64,
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
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
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
        messages_sent: Arc::new(tokio::sync::atomic::AtomicU64::new(0)),
    };

    let app = Router::new()
        .route("/", get(ws_handler))
        .with_state(state.clone())
        .into_make_service_with_connect_info::<SocketAddr>();

    info!("Rust WS server start", { port = port });
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind ws port");
    axum::serve(listener, app).await.expect("start ws server");
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
            if sender.send(msg).await.is_err() {
                break;
            }
        }
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

    info!("Client connected"; "id" => id.to_string(), "name" => name.clone(), "ip" => addr.ip().to_string());
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
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Err(err) = process_message(&state, id, &client, text).await {
                    send_to_one(&client, &Outgoing::Error { message: err });
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
    broadcast(
        &state,
        &Outgoing::System {
            text: format!("{} heeft de chat verlaten.", name),
            at: now_ms(),
        },
        Some(id),
    )
    .await;

    send_task.abort();
    info!("Client disconnected"; "id" => id.to_string(), "name" => name, "ip" => addr.ip().to_string());
}

async fn process_message(state: &AppState, id: Uuid, client: &Client, text: String) -> Result<(), String> {
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
            state.messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            broadcast(
                state,
                &Outgoing::Chat {
                    from: client.name.clone(),
                    text: trimmed.to_string(),
                    at: now_ms(),
                },
                None,
            )
            .await;
            info!("Bericht verzonden"; "from" => client.name.clone(), "id" => id.to_string(), "ip" => client.ip.clone());
        }
        Incoming::SetName { name } => {
            let trimmed = name.trim();
            if trimmed.len() < 2 || trimmed.len() > 32 {
                return Err("Naam moet tussen 2 en 32 tekens zijn.".into());
            }
            {
                let mut clients = state.clients.lock().await;
                if let Some(entry) = clients.get_mut(&id) {
                    let old = entry.name.clone();
                    entry.name = trimmed.to_string();
                    send_to_one(entry, &Outgoing::AckName { name: entry.name.clone(), at: now_ms() });
                    broadcast(
                        state,
                        &Outgoing::System {
                            text: format!("{old} heet nu {}.", entry.name),
                            at: now_ms(),
                        },
                        Some(id),
                    )
                    .await;
                    info!("Gebruikersnaam gewijzigd"; "old" => old, "new" => entry.name.clone(), "id" => id.to_string(), "ip" => entry.ip.clone());
                }
            }
        }
        Incoming::Status => {
            let uptime = state.started_at.elapsed().as_secs_f64();
            let count = state.clients.lock().await.len();
            let messages = state.messages_sent.load(std::sync::atomic::Ordering::Relaxed);
            send_to_one(
                client,
                &Outgoing::Status {
                    uptimeSeconds: uptime,
                    userCount: count,
                    messagesSent: messages,
                },
            );
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
            send_to_one(client, &Outgoing::ListUsers { users });
        }
    }

    Ok(())
}

async fn broadcast(state: &AppState, payload: &Outgoing, except: Option<Uuid>) {
    let text = serde_json::to_string(payload).unwrap_or_else(|_| "{\"type\":\"error\",\"message\":\"serialize\"}".into());
    let clients = state.clients.lock().await;
    for (id, client) in clients.iter() {
        if except.is_some() && except.unwrap() == *id {
            continue;
        }
        let _ = client.tx.send(Message::Text(text.clone()));
    }
}

fn send_to_one(client: &Client, payload: &Outgoing) {
    if let Ok(text) = serde_json::to_string(payload) {
        let _ = client.tx.send(Message::Text(text));
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
