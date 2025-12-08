# cbxchat

Real-time chat server with a Bun/Express frontend and high-performance Rust WebSocket backend.

## Overview

| Component | Path | Description |
|-----------|------|-------------|
| HTTP Server | `index.ts` | Express frontend, serves chat UI |
| WS Backend | `rust-ws/` | Rust/Axum WebSocket server (production) |
| CLI Client | `rust-client/` | Terminal chat client |
| GUI Client | `rust-gui/` | Graphical client (egui) |
| Health Monitor | `rust-wsmonitor/` | Health check tool |
| Benchmark | `rust-wsbench/` | Load testing tool |
| Bun WS | `ws-server.ts` | TypeScript backend (deprecated) |

---

## Installation

```bash
bun install
```

## Running

HTTP (frontend + status):
```bash
bun run index.ts
# or: bun run start:http
```

WebSocket backend (Rust):
```bash
cd rust-ws
cargo run --release
```

During development:
```bash
bun run dev:http              # frontend with auto-reload
cd rust-ws && cargo run       # Rust backend
```

## Configuration
- `PORT` (default `3000`): HTTP server.
- `HOST` (default `0.0.0.0`): HTTP server bind address.
- `WS_PORT` (default `3001`): WebSocket server.
- `WS_HOST`: optional hostname/IP for WebSocket URL (useful behind reverse proxy).
- `WS_URL`: optional full WebSocket URL; otherwise the frontend uses the host from the HTTP request + `WS_PORT` (useful for LAN clients to avoid connecting to `localhost`).
- `LOG_TARGET`: `stdout` (default) or `file`.
- `LOG_FILE`: path when `LOG_TARGET=file` or with `--log=file:path`.
- CLI: `--log=stdout` or `--log=file:server.log` works on both entrypoints.

## Routes
- `/` - chat frontend (index + JS/CSS).
- `/status` - JSON with runtime info: Bun version, environment, ports, uptime, requests, memory.

## WebSocket Contract
- Inbound (client → server):
  - `{ type: "chat", text }`
  - `{ type: "setName", name }`
  - `{ type: "status" }`
  - `{ type: "listUsers" }`
  - `{ type: "ping", token? }` — optional token for response validation
  - `{ type: "ai", prompt }` — ask AI a question ¹
- Outbound (server → client):
  - `chat` `{ from, text, at }`
  - `system` `{ text, at }`
  - `ackName` `{ name, at }`
  - `status` `{ uptimeSeconds, userCount, messagesSent, messagesPerSecond, memoryMb }` ²
  - `listUsers` `{ users: [{ id, name, ip }] }` ²
  - `pong` `{ token?, at }` — response to ping with the same token
  - `ai` `{ from, prompt, response, at }` — AI response broadcast ¹
  - `error` `{ message }`

¹ Rust backend only, requires AI configuration
² Rust backend only: `messagesPerSecond`, `memoryMb`, and `ip` fields

## Frontend Commands
- `/name new_name` — change username.
- `/status` — request server status.
- `/users` — list current users.
- `/ping [token]` — measure roundtrip time to server.
- `/ai <question>` — ask AI a question (requires configuration).

## Logging
Logging goes to stdout unless `LOG_TARGET=file` or `--log=file:path` is set. Backend logs join/leave/message events; HTTP logs startup info.

## Rust WebSocket Backend

The WebSocket backend is written in Rust with Axum and Tokio.

- **Location:** `rust-ws/`
- **Requirements:** Rust + Cargo

### Logging

By default, logging is disabled for maximum performance. Use `RUST_LOG` to enable logging:

```bash
# No logging (default) — for benchmarks and production
cargo run

# Info level: startup, connect/disconnect events
RUST_LOG=info cargo run

# Debug level: all messages and broadcasts
RUST_LOG=debug cargo run
```

### Configuration

The Rust backend reads the same `WS_PORT` environment variable as the HTTP server.

### Docker

The Rust backend can also run in a container:

```bash
cd rust-ws

# Build
docker build -t cbxchat-ws .

# Run
docker run -p 3001:3001 cbxchat-ws

# With logging
docker run -p 3001:3001 -e RUST_LOG=info cbxchat-ws

# Different port
docker run -p 8080:8080 -e WS_PORT=8080 cbxchat-ws
```

The image uses a multi-stage build (~15MB) with Alpine Linux.

## AI Integration

The server supports AI-powered Q&A via OpenRouter. Questions asked with `/ai` are sent to the AI and responses are broadcast to all users.

### Setup

1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Get an API key from [OpenRouter](https://openrouter.ai/keys)

3. Edit `.env`:
   ```bash
   OPENROUTER_API_KEY=sk-or-v1-your-key-here
   AI_ENABLED=true
   AI_MODEL=openai/gpt-4o
   AI_RATE_LIMIT=5
   ```

### Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENROUTER_API_KEY` | - | Required. Your OpenRouter API key |
| `AI_ENABLED` | `false` | Enable/disable AI feature |
| `AI_MODEL` | `openai/gpt-4o` | Model to use (see [OpenRouter models](https://openrouter.ai/models)) |
| `AI_RATE_LIMIT` | `5` | Max requests per user per minute |

### Usage

```
/ai What is the meaning of life?
```

The question and AI response are broadcast to all connected users.

## Bun/TypeScript WebSocket Backend (deprecated)

A Bun/TypeScript version of the WebSocket backend is also available for testing.

```bash
bun run ws-server.ts
# or: bun run start:ws

# With auto-reload:
bun run dev:ws
```

This version has the same protocol as the Rust backend, but lower performance (see Benchmarking).

## Native Clients

Two native Rust clients available:

### CLI Client

Command-line chat client for terminal use.

```bash
cd rust-client
cargo build --release
./target/release/chat                    # local
./target/release/chat ws://server:3001   # remote
```

Commands: `/name`, `/status`, `/users`, `/ping`, `/help`, `/quit`

Features:
- Command history with arrow keys (↑/↓)
- Cursor navigation (←/→)

### GUI Client

Graphical chat client built with egui.

```bash
cd rust-gui
cargo build --release
./target/release/chat-gui
```

Features:
- Configurable server URL
- Connect/disconnect button
- Send messages with Enter
- Commands: `/name`, `/status`, `/users`, `/ping`

### Health Monitor (wsmonitor)

CLI tool for health checks and monitoring in scripts.

```bash
cd rust-wsmonitor
cargo build --release
```

#### Usage

```bash
# Silent health check (for scripts)
./target/release/wsmonitor && echo "OK" || echo "FAIL"

# Verbose with roundtrip time
./target/release/wsmonitor -v
# PING ws://127.0.0.1:3001 (1 pings)
# seq=1: time=0.25ms

# Multiple pings with statistics
./target/release/wsmonitor -v --count=4
# PING ws://127.0.0.1:3001 (4 pings)
# seq=1: time=0.25ms
# seq=2: time=0.28ms
# seq=3: time=0.31ms
# seq=4: time=0.27ms
#
# --- ws://127.0.0.1:3001 ping statistics ---
# 4 pings, 4 received, 0% loss
# rtt min/avg/max = 0.25/0.28/0.31 ms

# Custom server
./target/release/wsmonitor -v ws://server:3001
```

#### Options

| Option | Description |
|--------|-------------|
| `-v`, `--verbose` | Show response times |
| `-c<N>`, `--count=<N>` | Number of pings (default: 1) |
| `-h`, `--help` | Show help |

#### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All pings successful |
| `1` | Connection or ping failed |

#### Script Examples

```bash
# Cron health check
*/5 * * * * /path/to/wsmonitor || echo "WS server down" | mail -s "Alert" admin@example.com

# Wait until server is available
until wsmonitor; do sleep 1; done && echo "Server is up"

# Monitoring with output
wsmonitor -v --count=10 | tee -a /var/log/ws-health.log
```

## Testing with websocat

Install [websocat](https://github.com/vi/websocat) to manually test the WebSocket backend:

```bash
# macOS
brew install websocat

# or via cargo
cargo install websocat
```

### Example Session

```bash
$ websocat -t ws://127.0.0.1:3001
{"type":"ackName","name":"guest-a1b2c3","at":1733312400000}
{"type":"status"}
{"type":"status","uptimeSeconds":42.5,"userCount":1,"messagesSent":0}
{"type":"chat","text":"Hello!"}
{"type":"chat","from":"guest-a1b2c3","text":"Hello!","at":1733312410000}
{"type":"setName","name":"Bas"}
{"type":"ackName","name":"Bas","at":1733312420000}
{"type":"listUsers"}
{"type":"listUsers","users":[{"id":"a1b2c3d4-...","name":"Bas"}]}
```

Lines without `$` prefix are server responses; lines you type are JSON requests.

## Benchmarking

Stress test tools to test the WebSocket backend.

### Rust Benchmark (recommended)

Native Rust benchmark for maximum performance and high client counts.

```bash
cd rust-wsbench
cargo build --release

# Basic test
./target/release/wsbench --clients=100 --rate=120 --duration=60

# High load test
./target/release/wsbench --clients=500 --rate=600 --duration=60

# Help
./target/release/wsbench --help
```

### Bun/TypeScript Benchmark

Two versions available in `tools/`:
- `ws-benchmark.ts` — Worker threads (max ~100 clients due to Bun bug)
- `ws-benchmark-async.ts` — Async (more stable for higher loads)

```bash
bun run tools/ws-benchmark-async.ts --clients=200 --rate=600 --duration=60
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--url` | `ws://127.0.0.1:3001` | WebSocket server URL |
| `--clients` | `10` | Number of concurrent clients |
| `--rate` | `60` | Messages per minute per client |
| `--duration` | `30` | Test duration in seconds |
| `--quiet` | `false` | Show only final results |

### Output

The benchmark shows:
- Live progress (connected clients, sent/received messages)
- Total sent/received messages
- Throughput (msg/s)
- Latency statistics (average, P50, P95, P99)

**Tip:** Increase the file descriptor limit for high client counts:
```bash
ulimit -n 10000  # in both terminals (server + benchmark)
```

### Benchmark Results (Rust backend)

Tested with `rust-wsbench` → `rust-ws` on Apple M1 Pro:

#### Many clients, low rate (broadcast-bound)

| Clients | Rate/min | Throughput | P50 | P95 | P99 | Status |
|---------|----------|------------|-----|-----|-----|--------|
| 200 | 200 | 664 msg/s | 1ms | 4ms | 4ms | ✓ Excellent |
| 500 | 120 | 995 msg/s | 3ms | 5ms | 7ms | ✓ Excellent |
| 1000 | 60 | 990 msg/s | 9ms | 18ms | 91ms | ✓ Good |
| 1500 | 30 | 744 msg/s | 16ms | 285ms | 1054ms | ⚠️ Limit |
| 2000 | 30 | 983 msg/s | 10s | 20s | 21s | ❌ Overloaded |

#### Few clients, high rate (throughput-bound)

| Clients | Rate/min | Throughput | P50 | P99 | Status |
|---------|----------|------------|-----|-----|--------|
| 10 | 30000 | 2968 msg/s | 0ms | 2ms | ✓ Excellent |
| 10 | 60000 | 4356 msg/s | 0ms | 2ms | ✓ Excellent |
| 10 | 75000 | overflow | 5.5s | 5.9s | ❌ Overloaded |

**Server limits:**

| Bottleneck | Limit | Scenario |
|------------|-------|----------|
| Max clients | 2000+ | No connection issues |
| Max inbound msg/s | ~5000 | Few clients, high rate |
| Max broadcasts/s | ~1M | Many clients, low rate |
| Sweet spot | 1000 clients @ 60/min | <100ms P99 |

### Rust vs Bun Backend

Direct comparison with 500 clients @ 120 msg/min:

| Metric | Rust | Bun | Difference |
|--------|------|-----|------------|
| Clients connected | 500/500 (100%) | 382/500 (76%) | Rust +31% |
| P50 latency | **3ms** | 8078ms | Rust 2700× faster |
| P95 latency | **5ms** | 32224ms | Rust 6400× faster |
| P99 latency | **7ms** | 39556ms | Rust 5600× faster |
| Errors | 0 | 0 | - |

**Conclusion:** The Rust backend delivers instant message delivery (<10ms) where the Bun backend has 8-40 seconds delay under the same load. This is why Rust is the default backend.

## CI/CD

GitHub Actions workflows run automatically on push and pull requests.

| Workflow | Trigger | Checks |
|----------|---------|--------|
| `ci-frontend.yml` | `*.ts`, `*.json`, `public/**` | Bun install, TypeScript typecheck, HTTP server startup |
| `ci-rust.yml` | `rust-*/**` | `cargo fmt`, `cargo clippy`, `cargo build` for all Rust projects |
| `integration-test.yml` | All pushes/PRs | Builds WS server + wsmonitor, runs health check |

### Running Locally

```bash
# Frontend checks
bun install
bun x tsc --noEmit

# Rust checks (per project)
cd rust-ws
cargo fmt --check
cargo clippy -- -D warnings
cargo build --release

# Integration test
./rust-ws/target/release/rust-ws &
sleep 2
./rust-wsmonitor/target/release/wsmonitor -v --count=5
```
