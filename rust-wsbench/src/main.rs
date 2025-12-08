use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

#[derive(Parser, Debug)]
#[command(name = "wsbench")]
#[command(about = "WebSocket benchmark tool for stress testing")]
struct Args {
    /// WebSocket server URL
    #[arg(long, default_value = "ws://127.0.0.1:3001")]
    url: String,

    /// Number of concurrent clients
    #[arg(long, default_value = "10")]
    clients: usize,

    /// Messages per minute per client
    #[arg(long, default_value = "60")]
    rate: u32,

    /// Test duration in seconds
    #[arg(long, default_value = "30")]
    duration: u64,

    /// Only show summary
    #[arg(long, default_value = "false")]
    quiet: bool,

    /// Flood mode: send as fast as possible (ignores --rate)
    #[arg(long, default_value = "false")]
    flood: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Outgoing {
    #[serde(rename = "chat")]
    Chat { text: String },
    #[serde(rename = "setName")]
    SetName { name: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Incoming {
    #[serde(rename = "chat")]
    Chat { from: String, text: String },
    #[serde(rename = "ackName")]
    AckName { name: String },
    #[serde(other)]
    Other,
}

const PHRASES: &[&str] = &[
    "Hallo, hoe gaat het?",
    "De zon schijnt vandaag!",
    "Wat een mooi weer.",
    "Ik ben aan het testen.",
    "Dit is een benchmark bericht.",
    "Hello from the benchmark tool!",
    "Testing WebSocket performance.",
    "Random message number",
    "How fast can we go?",
    "Stress testing in progress...",
    "The quick brown fox jumps over the lazy dog.",
    "Lorem ipsum dolor sit amet.",
    "WebSocket verbinding werkt prima.",
    "Server response time check.",
    "Latency measurement ongoing.",
];

fn random_phrase() -> String {
    let mut rng = rand::rng();
    let phrase = PHRASES[rng.random_range(0..PHRASES.len())];
    let suffix: String = (0..6)
        .map(|_| rng.random_range(b'a'..=b'z') as char)
        .collect();
    format!("{}{}", phrase, suffix)
}

fn random_interval(base_us: u64) -> Duration {
    if base_us == 0 {
        return Duration::ZERO;
    }
    let mut rng = rand::rng();
    let variance = (base_us as f64 * 0.3) as i64;
    let offset = rng.random_range(-variance..=variance);
    Duration::from_micros((base_us as i64 + offset).max(100) as u64)
}

struct Stats {
    connected: AtomicU64,
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    errors: AtomicU64,
    latencies: Mutex<Vec<u64>>,
}

impl Stats {
    fn new() -> Self {
        Self {
            connected: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            latencies: Mutex::new(Vec::new()),
        }
    }
}

async fn run_client(
    client_id: usize,
    url: String,
    rate: u32,
    end_time: Instant,
    stats: Arc<Stats>,
    quiet: bool,
    flood: bool,
) {
    let name = format!("bench-{}", client_id);
    // Calculate interval in microseconds: 60 seconds = 60_000_000 microseconds
    let base_interval_us = if flood {
        0
    } else {
        60_000_000 / rate.max(1) as u64
    };

    // Connect
    let ws_stream = match tokio_tungstenite::connect_async(&url).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            if !quiet {
                eprintln!("[Client {}] Connection failed: {}", client_id, e);
            }
            stats.errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    stats.connected.fetch_add(1, Ordering::Relaxed);
    if !quiet {
        println!("[Client {}] Connected", client_id);
    }

    let (mut write, mut read) = ws_stream.split();

    // Send initial name
    let set_name = serde_json::to_string(&Outgoing::SetName { name: name.clone() }).unwrap();
    if write.send(Message::Text(set_name.into())).await.is_err() {
        stats.errors.fetch_add(1, Ordering::Relaxed);
        stats.connected.fetch_sub(1, Ordering::Relaxed);
        return;
    }

    // Track pending messages for latency
    let pending: Arc<RwLock<HashMap<String, Instant>>> = Arc::new(RwLock::new(HashMap::new()));
    let pending_read = pending.clone();
    let stats_read = stats.clone();
    let client_name = Arc::new(RwLock::new(name.clone()));
    let client_name_read = client_name.clone();

    // Reader task
    let reader = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    stats_read.messages_received.fetch_add(1, Ordering::Relaxed);

                    if let Ok(incoming) = serde_json::from_str::<Incoming>(&text) {
                        match incoming {
                            Incoming::AckName { name } => {
                                *client_name_read.write().await = name;
                            }
                            Incoming::Chat { from, text } => {
                                let current_name = client_name_read.read().await.clone();
                                if from == current_name {
                                    if let Some(msg_id) = text.split('|').next() {
                                        let mut pending = pending_read.write().await;
                                        if let Some(sent_at) = pending.remove(msg_id) {
                                            let latency = sent_at.elapsed().as_millis() as u64;
                                            stats_read.latencies.lock().await.push(latency);
                                        }
                                    }
                                }
                            }
                            Incoming::Other => {}
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });

    // Writer loop
    let mut msg_count = 0u64;
    while Instant::now() < end_time {
        let msg_id = format!("{}-{}", client_id, msg_count);
        let text = format!("{}|{}", msg_id, random_phrase());

        pending.write().await.insert(msg_id, Instant::now());

        let chat = serde_json::to_string(&Outgoing::Chat { text }).unwrap();
        if write.send(Message::Text(chat.into())).await.is_err() {
            stats.errors.fetch_add(1, Ordering::Relaxed);
            break;
        }

        stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        msg_count += 1;

        let interval = random_interval(base_interval_us);
        if interval.is_zero() {
            tokio::task::yield_now().await;
        } else {
            tokio::time::sleep(interval).await;
        }
    }

    // Close connection
    let _ = write.send(Message::Close(None)).await;
    reader.abort();
    stats.connected.fetch_sub(1, Ordering::Relaxed);
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[idx.saturating_sub(1).min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let rate_display = if args.flood {
        "FLOOD (max speed)".to_string()
    } else {
        format!("{} msg/min/client", args.rate)
    };

    println!(
        r#"
WebSocket Benchmark (Rust)
═══════════════════════════════════════
URL:        {}
Clients:    {}
Rate:       {}
Duration:   {}s
═══════════════════════════════════════
"#,
        args.url, args.clients, rate_display, args.duration
    );

    let stats = Arc::new(Stats::new());
    let start_time = Instant::now();
    let end_time = start_time + Duration::from_secs(args.duration);

    // Connect clients in batches
    println!("Connecting clients...");
    let mut handles = Vec::new();
    let batch_size = 50;
    let batch_delay = Duration::from_millis(100);

    for batch_start in (0..args.clients).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(args.clients);

        for client_id in batch_start..batch_end {
            let url = args.url.clone();
            let stats = stats.clone();
            let quiet = args.quiet;
            let flood = args.flood;
            let rate = args.rate;

            handles.push(tokio::spawn(async move {
                run_client(client_id, url, rate, end_time, stats, quiet, flood).await;
            }));
        }

        if batch_end < args.clients {
            tokio::time::sleep(batch_delay).await;
        }
    }

    // Wait a moment for connections
    tokio::time::sleep(Duration::from_millis(500)).await;

    let connected = stats.connected.load(Ordering::Relaxed);
    println!("Connected: {}/{}\n", connected, args.clients);

    // Progress indicator
    let stats_progress = stats.clone();
    let duration = args.duration;
    let total_clients = args.clients;
    let progress_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let start = Instant::now();

        loop {
            interval.tick().await;
            let elapsed = start.elapsed().as_secs();
            if elapsed >= duration {
                break;
            }

            let connected = stats_progress.connected.load(Ordering::Relaxed);
            let sent = stats_progress.messages_sent.load(Ordering::Relaxed);
            let recv = stats_progress.messages_received.load(Ordering::Relaxed);

            println!(
                "[{}s/{}s] Connected: {}/{} | Sent: {} | Recv: {}",
                elapsed, duration, connected, total_clients, sent, recv
            );
        }
    });

    // Wait for all clients to finish
    for handle in handles {
        let _ = handle.await;
    }

    progress_handle.abort();

    // Calculate final stats
    let total_sent = stats.messages_sent.load(Ordering::Relaxed);
    let total_recv = stats.messages_received.load(Ordering::Relaxed);
    let total_errors = stats.errors.load(Ordering::Relaxed);

    let mut latencies = stats.latencies.lock().await;
    latencies.sort_unstable();

    let avg_latency = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
    };

    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);

    let throughput = total_sent as f64 / args.duration as f64;

    println!(
        r#"
═══════════════════════════════════════
Results
═══════════════════════════════════════
Clients connected:  {}/{}
Messages sent:      {}
Messages received:  {}
Errors:             {}
Throughput:         {:.1} msg/s

Latency (ms):
  Average:  {:.2}
  P50:      {}
  P95:      {}
  P99:      {}
═══════════════════════════════════════
"#,
        connected,
        args.clients,
        total_sent,
        total_recv,
        total_errors,
        throughput,
        avg_latency,
        p50,
        p95,
        p99
    );
}
