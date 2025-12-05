use std::io::{self, BufRead, Write};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Outgoing {
    #[serde(rename = "chat")]
    Chat { text: String },
    #[serde(rename = "setName")]
    SetName { name: String },
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "listUsers")]
    ListUsers,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Incoming {
    #[serde(rename = "chat")]
    Chat { from: String, text: String },
    #[serde(rename = "system")]
    System { text: String },
    #[serde(rename = "ackName")]
    AckName { name: String },
    #[serde(rename = "status")]
    Status {
        #[serde(rename = "uptimeSeconds")]
        uptime_seconds: u64,
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

#[derive(Debug, Deserialize)]
struct UserInfo {
    name: String,
}

fn print_help() {
    println!("\x1b[90m");
    println!("Commands:");
    println!("  /name <username>  Change your username");
    println!("  /status           Show server status");
    println!("  /users            List connected users");
    println!("  /help             Show this help");
    println!("  /quit             Exit the client");
    println!("\x1b[0m");
}

fn format_message(msg: &Incoming) -> String {
    match msg {
        Incoming::Chat { from, text } => format!("\x1b[1m{}\x1b[0m: {}", from, text),
        Incoming::System { text } => format!("\x1b[33m* {}\x1b[0m", text),
        Incoming::AckName { name } => format!("\x1b[32m✓ Your name is now: {}\x1b[0m", name),
        Incoming::Status { uptime_seconds, user_count, messages_sent } => {
            format!(
                "\x1b[36m[Status] uptime: {}s, users: {}, messages: {}\x1b[0m",
                uptime_seconds, user_count, messages_sent
            )
        }
        Incoming::ListUsers { users } => {
            let names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
            format!("\x1b[36m[Users] {}\x1b[0m", names.join(", "))
        }
        Incoming::Error { message } => format!("\x1b[31m✗ Error: {}\x1b[0m", message),
    }
}

fn parse_command(input: &str) -> Option<Outgoing> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if input.starts_with('/') {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match cmd.as_str() {
            "/name" => {
                if arg.is_empty() {
                    println!("\x1b[31mUsage: /name <username>\x1b[0m");
                    None
                } else {
                    Some(Outgoing::SetName { name: arg.to_string() })
                }
            }
            "/status" => Some(Outgoing::Status),
            "/users" => Some(Outgoing::ListUsers),
            "/help" => {
                print_help();
                None
            }
            "/quit" | "/exit" | "/q" => {
                std::process::exit(0);
            }
            _ => {
                println!("\x1b[31mUnknown command: {}\x1b[0m", cmd);
                None
            }
        }
    } else {
        Some(Outgoing::Chat { text: input.to_string() })
    }
}

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:3001".to_string());

    println!("\x1b[90mConnecting to {}...\x1b[0m", url);

    let (ws_stream, _) = match connect_async(&url).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("\x1b[31mFailed to connect: {}\x1b[0m", e);
            std::process::exit(1);
        }
    };

    println!("\x1b[32mConnected!\x1b[0m Type /help for commands.\n");

    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Outgoing>();

    // Spawn stdin reader
    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(input) => {
                    if let Some(msg) = parse_command(&input) {
                        if tx_clone.send(msg).is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
            // Re-print prompt
            print!("> ");
            let _ = io::stdout().flush();
        }
    });

    // Print initial prompt
    print!("> ");
    let _ = io::stdout().flush();

    loop {
        tokio::select! {
            // Receive from server
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Clear current line and print message
                        print!("\r\x1b[K");
                        if let Ok(incoming) = serde_json::from_str::<Incoming>(&text) {
                            println!("{}", format_message(&incoming));
                        } else {
                            println!("\x1b[90m{}\x1b[0m", text);
                        }
                        print!("> ");
                        let _ = io::stdout().flush();
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        println!("\n\x1b[33mDisconnected from server\x1b[0m");
                        break;
                    }
                    Some(Err(e)) => {
                        println!("\n\x1b[31mConnection error: {}\x1b[0m", e);
                        break;
                    }
                    _ => {}
                }
            }
            // Send to server
            Some(msg) = rx.recv() => {
                let json = serde_json::to_string(&msg).unwrap();
                if write.send(Message::Text(json.into())).await.is_err() {
                    println!("\n\x1b[31mFailed to send message\x1b[0m");
                    break;
                }
            }
        }
    }
}
