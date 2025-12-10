use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;
use crossterm::{cursor, execute};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const MAX_HISTORY: usize = 20;

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
    #[serde(rename = "ping")]
    Ping { token: Option<String> },
    #[serde(rename = "ai")]
    Ai { prompt: String },
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
        version: String,
        #[serde(rename = "uptimeSeconds")]
        uptime_seconds: u64,
        #[serde(rename = "userCount")]
        user_count: usize,
        #[serde(rename = "messagesSent")]
        messages_sent: u64,
        #[serde(rename = "messagesPerSecond")]
        messages_per_second: f64,
        #[serde(rename = "memoryMb")]
        memory_mb: f64,
    },
    #[serde(rename = "listUsers")]
    ListUsers { users: Vec<UserInfo> },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong { token: Option<String> },
    #[serde(rename = "ai")]
    Ai {
        from: String,
        prompt: String,
        response: String,
        #[serde(rename = "responseMs")]
        response_ms: u64,
        tokens: Option<u32>,
        cost: Option<f64>,
    },
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    id: String,
    name: String,
    ip: String,
}

fn print_help() {
    print!("\x1b[90m\r\n");
    print!("Commands:\r\n");
    print!("  /name <username>  Change your username\r\n");
    print!("  /status           Show server status\r\n");
    print!("  /users            List connected users\r\n");
    print!("  /ping [token]     Ping server (measures roundtrip)\r\n");
    print!("  /ai <question>    Ask AI a question\r\n");
    print!("  /help             Show this help\r\n");
    print!("  /quit             Exit the client\r\n");
    print!("\x1b[0m\r\n");
    let _ = io::stdout().flush();
}

fn format_message(msg: &Incoming) -> String {
    match msg {
        Incoming::Chat { from, text } => format!("\x1b[1m{}\x1b[0m: {}", from, text),
        Incoming::System { text } => format!("\x1b[33m* {}\x1b[0m", text),
        Incoming::AckName { name } => format!("\x1b[32m✓ Your name is now: {}\x1b[0m", name),
        Incoming::Status {
            version,
            uptime_seconds,
            user_count,
            messages_sent,
            messages_per_second,
            memory_mb,
        } => {
            format!(
                "\x1b[36m[Status] v{} | users: {} | uptime: {}s | msgs: {} | msg/s: {} | mem: {:.2} MB\x1b[0m",
                version, user_count, uptime_seconds, messages_sent, messages_per_second, memory_mb
            )
        }
        Incoming::ListUsers { users } => {
            if users.is_empty() {
                return "\x1b[36m[Users] No users connected\x1b[0m".to_string();
            }
            // Calculate column widths
            let name_width = users.iter().map(|u| u.name.len()).max().unwrap_or(4).max(4);
            let ip_width = users.iter().map(|u| u.ip.len()).max().unwrap_or(2).max(2);

            let mut output = String::from("\x1b[36m");
            output.push_str(&format!(
                "\r\n  {:<name_width$}  {:<ip_width$}  {}\r\n",
                "NAME", "IP", "ID"
            ));
            output.push_str(&format!(
                "  {:-<name_width$}  {:-<ip_width$}  {:-<36}\r\n",
                "", "", ""
            ));
            for u in users {
                output.push_str(&format!(
                    "  {:<name_width$}  {:<ip_width$}  {}\r\n",
                    u.name, u.ip, u.id
                ));
            }
            output.push_str("\x1b[0m");
            output
        }
        Incoming::Error { message } => format!("\x1b[31m✗ Error: {}\x1b[0m", message),
        Incoming::Pong { token } => {
            let token_str = token
                .as_ref()
                .map(|t| format!(" (token: {}...)", &t[..8.min(t.len())]))
                .unwrap_or_default();
            format!("\x1b[36m[Pong]{}\x1b[0m", token_str)
        }
        Incoming::Ai {
            from,
            prompt,
            response,
            response_ms,
            tokens,
            cost,
        } => {
            let mut stats = vec![format!("{}ms", response_ms)];
            if let Some(t) = tokens {
                stats.push(format!("{} tokens", t));
            }
            if let Some(c) = cost {
                stats.push(format!("${:.4}", c));
            }
            format!(
                "\x1b[35m[AI] {}\x1b[0m asked: {} \x1b[90m({})\x1b[0m\r\n\x1b[36m{}\x1b[0m",
                from,
                prompt,
                stats.join(" | "),
                response
            )
        }
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
                    print!("\x1b[31mUsage: /name <username>\x1b[0m\r\n");
                    let _ = io::stdout().flush();
                    None
                } else {
                    Some(Outgoing::SetName {
                        name: arg.to_string(),
                    })
                }
            }
            "/status" => Some(Outgoing::Status),
            "/users" => Some(Outgoing::ListUsers),
            "/ping" => {
                let token = if arg.is_empty() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    arg.to_string()
                };
                Some(Outgoing::Ping { token: Some(token) })
            }
            "/ai" => {
                if arg.is_empty() {
                    print!("\x1b[31mUsage: /ai <question>\x1b[0m\r\n");
                    let _ = io::stdout().flush();
                    None
                } else {
                    print!("\x1b[90mAI is thinking...\x1b[0m\r\n");
                    let _ = io::stdout().flush();
                    Some(Outgoing::Ai {
                        prompt: arg.to_string(),
                    })
                }
            }
            "/help" => {
                print_help();
                None
            }
            "/quit" | "/exit" | "/q" => {
                let _ = terminal::disable_raw_mode();
                std::process::exit(0);
            }
            _ => {
                print!("\x1b[31mUnknown command: {}\x1b[0m\r\n", cmd);
                let _ = io::stdout().flush();
                None
            }
        }
    } else {
        Some(Outgoing::Chat {
            text: input.to_string(),
        })
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

    println!("\x1b[32mConnected!\x1b[0m Type /help for commands.");

    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Outgoing>();
    let pending_pings: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_pings_clone = Arc::clone(&pending_pings);

    // Spawn stdin reader with command history
    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let _ = terminal::enable_raw_mode();

        let mut history: Vec<String> = Vec::new();
        let mut history_idx: Option<usize> = None;
        let mut input = String::new();
        let mut cursor_pos: usize = 0; // char index, not byte index

        // Helper to get byte index from char index
        let char_to_byte = |s: &str, char_idx: usize| -> usize {
            s.char_indices()
                .nth(char_idx)
                .map(|(i, _)| i)
                .unwrap_or(s.len())
        };

        // Helper to get char count
        let char_count = |s: &str| -> usize { s.chars().count() };

        loop {
            if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key_event)) = event::read() {
                    match key_event.code {
                        KeyCode::Enter => {
                            print!("\r\n");
                            let _ = io::stdout().flush();

                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                // Save commands to history
                                if trimmed.starts_with('/') && history.last() != Some(&trimmed) {
                                    history.push(trimmed.clone());
                                    if history.len() > MAX_HISTORY {
                                        history.remove(0);
                                    }
                                }

                                if let Some(msg) = parse_command(&trimmed) {
                                    if tx_clone.send(msg).is_err() {
                                        break;
                                    }
                                }
                            }

                            input.clear();
                            cursor_pos = 0;
                            history_idx = None;

                            print!("> ");
                            let _ = io::stdout().flush();
                        }
                        KeyCode::Backspace => {
                            if cursor_pos > 0 {
                                let byte_pos = char_to_byte(&input, cursor_pos - 1);
                                let next_byte_pos = char_to_byte(&input, cursor_pos);
                                input.replace_range(byte_pos..next_byte_pos, "");
                                cursor_pos -= 1;
                                print!("\r\x1b[K> {}", input);
                                if cursor_pos < char_count(&input) {
                                    let _ = execute!(
                                        io::stdout(),
                                        cursor::MoveToColumn((cursor_pos + 2) as u16)
                                    );
                                }
                                let _ = io::stdout().flush();
                            }
                        }
                        KeyCode::Left => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                                let _ = execute!(io::stdout(), cursor::MoveLeft(1));
                                let _ = io::stdout().flush();
                            }
                        }
                        KeyCode::Right => {
                            if cursor_pos < char_count(&input) {
                                cursor_pos += 1;
                                let _ = execute!(io::stdout(), cursor::MoveRight(1));
                                let _ = io::stdout().flush();
                            }
                        }
                        KeyCode::Up => {
                            if !history.is_empty() {
                                let new_idx = match history_idx {
                                    None => history.len() - 1,
                                    Some(0) => 0,
                                    Some(i) => i - 1,
                                };
                                history_idx = Some(new_idx);
                                input = history[new_idx].clone();
                                cursor_pos = char_count(&input);
                                print!("\r\x1b[K> {}", input);
                                let _ = io::stdout().flush();
                            }
                        }
                        KeyCode::Down => {
                            match history_idx {
                                Some(i) if i + 1 < history.len() => {
                                    history_idx = Some(i + 1);
                                    input = history[i + 1].clone();
                                    cursor_pos = char_count(&input);
                                }
                                Some(_) => {
                                    history_idx = None;
                                    input.clear();
                                    cursor_pos = 0;
                                }
                                None => {}
                            }
                            print!("\r\x1b[K> {}", input);
                            let _ = io::stdout().flush();
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            let _ = terminal::disable_raw_mode();
                            println!("\r");
                            std::process::exit(0);
                        }
                        KeyCode::Char(c) => {
                            let byte_pos = char_to_byte(&input, cursor_pos);
                            input.insert(byte_pos, c);
                            cursor_pos += 1;
                            print!("\r\x1b[K> {}", input);
                            if cursor_pos < char_count(&input) {
                                let _ = execute!(
                                    io::stdout(),
                                    cursor::MoveToColumn((cursor_pos + 2) as u16)
                                );
                            }
                            let _ = io::stdout().flush();
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = terminal::disable_raw_mode();
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
                            // Handle Pong with roundtrip calculation
                            if let Incoming::Pong { ref token } = incoming {
                                let roundtrip = token.as_ref().and_then(|t| {
                                    pending_pings_clone.lock().ok()?.remove(t).map(|start| start.elapsed())
                                });
                                let token_str = token.as_ref().map(|t| format!(" (token: {}...)", &t[..8.min(t.len())])).unwrap_or_default();
                                if let Some(rtt) = roundtrip {
                                    print!("\x1b[36m[Pong] roundtrip: {:.2}ms{}\x1b[0m\r\n", rtt.as_secs_f64() * 1000.0, token_str);
                                } else {
                                    print!("{}\r\n", format_message(&incoming));
                                }
                            } else {
                                print!("{}\r\n", format_message(&incoming));
                            }
                        } else {
                            print!("\x1b[90m{}\x1b[0m\r\n", text);
                        }
                        print!("> ");
                        let _ = io::stdout().flush();
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        print!("\r\n\x1b[33mDisconnected from server\x1b[0m\r\n");
                        let _ = io::stdout().flush();
                        break;
                    }
                    Some(Err(e)) => {
                        print!("\r\n\x1b[31mConnection error: {}\x1b[0m\r\n", e);
                        let _ = io::stdout().flush();
                        break;
                    }
                    _ => {}
                }
            }
            // Send to server
            Some(msg) = rx.recv() => {
                // Store timestamp for ping messages
                if let Outgoing::Ping { token: Some(ref t) } = msg {
                    if let Ok(mut pings) = pending_pings.lock() {
                        pings.insert(t.clone(), Instant::now());
                    }
                }
                let json = serde_json::to_string(&msg).unwrap();
                if write.send(Message::Text(json.into())).await.is_err() {
                    print!("\r\n\x1b[31mFailed to send message\x1b[0m\r\n");
                    let _ = io::stdout().flush();
                    break;
                }
            }
        }
    }
}
