use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEFAULT_URL: &str = "ws://127.0.0.1:3001";

#[derive(Serialize)]
struct PingRequest {
    #[serde(rename = "type")]
    msg_type: &'static str,
    token: String,
}

#[derive(Deserialize)]
struct PongResponse {
    #[serde(rename = "type")]
    msg_type: String,
    token: Option<String>,
}

struct Args {
    url: String,
    verbose: bool,
    count: u32,
}

fn parse_args() -> Args {
    let mut args = Args {
        url: DEFAULT_URL.to_string(),
        verbose: false,
        count: 1,
    };

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-v" | "--print" | "--verbose" => args.verbose = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            s if s.starts_with("--count=") => {
                if let Ok(n) = s.trim_start_matches("--count=").parse() {
                    args.count = n;
                }
            }
            s if s.starts_with("-c") => {
                if let Ok(n) = s.trim_start_matches("-c").parse() {
                    args.count = n;
                }
            }
            s if !s.starts_with('-') => {
                args.url = s.to_string();
            }
            _ => {}
        }
    }

    args
}

fn print_help() {
    eprintln!("Usage: wsmonitor [OPTIONS] [URL]");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  [URL]  WebSocket server URL (default: {})", DEFAULT_URL);
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -v, --verbose      Print response times");
    eprintln!("  -c, --count=<N>    Number of pings to send (default: 1)");
    eprintln!("  -h, --help         Show this help");
    eprintln!();
    eprintln!("Exit codes:");
    eprintln!("  0  All pings successful");
    eprintln!("  1  Connection or ping failed");
}

#[tokio::main]
async fn main() {
    let args = parse_args();

    // Connect
    let (ws_stream, _) = match connect_async(&args.url).await {
        Ok(conn) => conn,
        Err(e) => {
            if args.verbose {
                eprintln!("Failed to connect to {}: {}", args.url, e);
            }
            std::process::exit(1);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    if args.verbose {
        eprintln!("PING {} ({} pings)", args.url, args.count);
    }

    let mut success_count = 0u32;
    let mut total_time = 0.0f64;
    let mut min_time = f64::MAX;
    let mut max_time = 0.0f64;

    for seq in 1..=args.count {
        let token = uuid::Uuid::new_v4().to_string();
        let ping = PingRequest {
            msg_type: "ping",
            token: token.clone(),
        };

        let start = Instant::now();

        // Send ping
        let json = serde_json::to_string(&ping).unwrap();
        if write.send(Message::Text(json.into())).await.is_err() {
            if args.verbose {
                eprintln!("seq={}: send failed", seq);
            }
            continue;
        }

        // Wait for pong with timeout
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            async {
                while let Some(msg) = read.next().await {
                    if let Ok(Message::Text(text)) = msg {
                        if let Ok(pong) = serde_json::from_str::<PongResponse>(&text) {
                            if pong.msg_type == "pong" && pong.token.as_ref() == Some(&token) {
                                return Some(start.elapsed());
                            }
                        }
                    }
                }
                None
            }
        ).await;

        match timeout {
            Ok(Some(elapsed)) => {
                let ms = elapsed.as_secs_f64() * 1000.0;
                success_count += 1;
                total_time += ms;
                min_time = min_time.min(ms);
                max_time = max_time.max(ms);

                if args.verbose {
                    println!("seq={}: time={:.2}ms", seq, ms);
                }
            }
            Ok(None) => {
                if args.verbose {
                    eprintln!("seq={}: connection closed", seq);
                }
                break;
            }
            Err(_) => {
                if args.verbose {
                    eprintln!("seq={}: timeout", seq);
                }
            }
        }

        // Small delay between pings
        if seq < args.count {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    // Print summary
    if args.verbose && args.count > 1 {
        let loss = ((args.count - success_count) as f64 / args.count as f64) * 100.0;
        eprintln!();
        eprintln!("--- {} ping statistics ---", args.url);
        eprintln!(
            "{} pings, {} received, {:.0}% loss",
            args.count, success_count, loss
        );
        if success_count > 0 {
            let avg = total_time / success_count as f64;
            eprintln!("rtt min/avg/max = {:.2}/{:.2}/{:.2} ms", min_time, avg, max_time);
        }
    }

    // Exit code
    if success_count == args.count {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
