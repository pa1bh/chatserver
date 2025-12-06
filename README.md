# bunserve

Chatserver met een Express/Bun frontend en een Rust WebSocket backend.

## Installeren

```bash
bun install
```

## Starten

HTTP (frontend + status):
```bash
bun run index.ts
# of: bun run start:http
```

WebSocket backend (Rust):
```bash
cd rust-ws
cargo run --release
```

Tijdens ontwikkeling:
```bash
bun run dev:http              # frontend met auto-reload
cd rust-ws && cargo run       # Rust backend
```

## Configuratie
- `PORT` (default `3000`): HTTP server.
- `HOST` (default `0.0.0.0`): Bind adres HTTP server.
- `WS_PORT` (default `3001`): WebSocket server.
- `WS_HOST`: optioneel hostnaam/IP voor de WebSocket URL (handig achter reverse proxy).
- `WS_URL`: optioneel volledige WebSocket-URL; anders gebruikt de frontend de host uit het HTTP-verzoek + `WS_PORT` (handig voor LAN-clients zodat er niet naar `localhost` wordt verbonden).
- `LOG_TARGET`: `stdout` (default) of `file`.
- `LOG_FILE`: pad als `LOG_TARGET=file` of bij `--log=file:pad`.
- CLI: `--log=stdout` of `--log=file:server.log` werkt op beide entrypoints.

## Routes
- `/` - chatfrontend (index + JS/CSS).
- `/status` - JSON met runtime info: Bun-versie, omgeving, ports, uptime, requests, memory.

## WebSocket contract
- Inbound (client → server):
  - `{ type: "chat", text }`
  - `{ type: "setName", name }`
  - `{ type: "status" }`
  - `{ type: "listUsers" }`
  - `{ type: "ping", token? }` — optionele token voor response validatie
- Outbound (server → client):
  - `chat` `{ from, text, at }`
  - `system` `{ text, at }`
  - `ackName` `{ name, at }`
  - `status` `{ uptimeSeconds, userCount, messagesSent, messagesPerSecond, memoryMb }` ¹
  - `listUsers` `{ users: [{ id, name, ip }] }` ¹
  - `pong` `{ token?, at }` — response op ping met dezelfde token
  - `error` `{ message }`

¹ Rust backend only: `messagesPerSecond`, `memoryMb`, en `ip` velden

## Frontend commands
- `/name nieuwe_naam` — wijzig gebruikersnaam.
- `/status` — vraag serverstatus op.
- `/users` — lijst huidige gebruikers.
- `/ping [token]` — meet roundtrip tijd naar server.

## Logging
Logging gaat naar stdout tenzij `LOG_TARGET=file` of `--log=file:pad` is gezet. Backend logt join/leave/bericht events; HTTP logt startinfo.

## Rust WebSocket backend

De WebSocket backend is geschreven in Rust met Axum en Tokio.

- **Locatie:** `rust-ws/`
- **Vereisten:** Rust + Cargo

### Logging

Standaard is logging uitgeschakeld voor maximale performance. Gebruik `RUST_LOG` om logging in te schakelen:

```bash
# Geen logging (default) — voor benchmarks en productie
cargo run

# Info level: startup, connect/disconnect events
RUST_LOG=info cargo run

# Debug level: alle berichten en broadcasts
RUST_LOG=debug cargo run
```

### Configuratie

De Rust backend leest dezelfde `WS_PORT` environment variable als de HTTP server.

### Docker

De Rust backend kan ook in een container draaien:

```bash
cd rust-ws

# Bouwen
docker build -t bunserve-ws .

# Starten
docker run -p 3001:3001 bunserve-ws

# Met logging
docker run -p 3001:3001 -e RUST_LOG=info bunserve-ws

# Andere poort
docker run -p 8080:8080 -e WS_PORT=8080 bunserve-ws
```

De image gebruikt een multi-stage build (~15MB) met Alpine Linux.

## Bun/TypeScript WebSocket backend (deprecated)

Er is ook een Bun/TypeScript versie van de WebSocket backend beschikbaar voor testen.

```bash
bun run ws-server.ts
# of: bun run start:ws

# Met auto-reload:
bun run dev:ws
```

Deze versie heeft hetzelfde protocol als de Rust backend, maar lagere performance (zie Benchmarking).

## Native Clients

Twee native Rust clients beschikbaar:

### CLI Client

Command-line chat client voor terminal gebruik.

```bash
cd rust-client
cargo build --release
./target/release/chat                    # lokaal
./target/release/chat ws://server:3001   # remote
```

Commands: `/name`, `/status`, `/users`, `/ping`, `/help`, `/quit`

Features:
- Command history met pijltjestoetsen (↑/↓)
- Cursor navigatie (←/→)

### GUI Client

Grafische chat client gebouwd met egui.

```bash
cd rust-gui
cargo build --release
./target/release/chat-gui
```

Features:
- Configureerbare server URL
- Connect/disconnect knop
- Berichten versturen met Enter
- Commands: `/name`, `/status`, `/users`, `/ping`

### Health Monitor (wsmonitor)

CLI tool voor health checks en monitoring in scripts.

```bash
cd rust-wsmonitor
cargo build --release
```

#### Gebruik

```bash
# Silent health check (voor scripts)
./target/release/wsmonitor && echo "OK" || echo "FAIL"

# Verbose met roundtrip tijd
./target/release/wsmonitor -v
# PING ws://127.0.0.1:3001 (1 pings)
# seq=1: time=0.25ms

# Meerdere pings met statistieken
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

#### Opties

| Optie | Beschrijving |
|-------|--------------|
| `-v`, `--verbose` | Toon response tijden |
| `-c<N>`, `--count=<N>` | Aantal pings (default: 1) |
| `-h`, `--help` | Help tonen |

#### Exit codes

| Code | Betekenis |
|------|-----------|
| `0` | Alle pings succesvol |
| `1` | Verbinding of ping gefaald |

#### Voorbeelden in scripts

```bash
# Cron health check
*/5 * * * * /path/to/wsmonitor || echo "WS server down" | mail -s "Alert" admin@example.com

# Wacht tot server beschikbaar is
until wsmonitor; do sleep 1; done && echo "Server is up"

# Monitoring met output
wsmonitor -v --count=10 | tee -a /var/log/ws-health.log
```

## Testen met websocat

Installeer [websocat](https://github.com/vi/websocat) om de WebSocket backend handmatig te testen:

```bash
# macOS
brew install websocat

# of via cargo
cargo install websocat
```

### Voorbeeldsessie

```bash
$ websocat -t ws://127.0.0.1:3001
{"type":"ackName","name":"guest-a1b2c3","at":1733312400000}
{"type":"status"}
{"type":"status","uptimeSeconds":42.5,"userCount":1,"messagesSent":0}
{"type":"chat","text":"Hallo!"}
{"type":"chat","from":"guest-a1b2c3","text":"Hallo!","at":1733312410000}
{"type":"setName","name":"Bas"}
{"type":"ackName","name":"Bas","at":1733312420000}
{"type":"listUsers"}
{"type":"listUsers","users":[{"id":"a1b2c3d4-...","name":"Bas"}]}
```

De regels zonder `$` prefix zijn server responses; regels die je zelf typt zijn JSON requests.

## Benchmarking

Stress test tools om de WebSocket backend te testen.

### Rust Benchmark (aanbevolen)

Native Rust benchmark voor maximale performance en hoge client counts.

```bash
cd rust-wsbench
cargo build --release

# Basis test
./target/release/wsbench --clients=100 --rate=120 --duration=60

# High load test
./target/release/wsbench --clients=500 --rate=600 --duration=60

# Help
./target/release/wsbench --help
```

### Bun/TypeScript Benchmark

Er zijn twee versies in `tools/`:
- `ws-benchmark.ts` — Worker threads (max ~100 clients door Bun bug)
- `ws-benchmark-async.ts` — Async (stabieler voor hogere loads)

```bash
bun run tools/ws-benchmark-async.ts --clients=200 --rate=600 --duration=60
```

### Opties

| Optie | Default | Beschrijving |
|-------|---------|--------------|
| `--url` | `ws://127.0.0.1:3001` | WebSocket server URL |
| `--clients` | `10` | Aantal concurrent clients |
| `--rate` | `60` | Berichten per minuut per client |
| `--duration` | `30` | Testduur in seconden |
| `--quiet` | `false` | Alleen eindresultaten tonen |

### Output

De benchmark toont:
- Live progress (connected clients, sent/received messages)
- Totaal verzonden/ontvangen berichten
- Throughput (msg/s)
- Latency statistieken (average, P50, P95, P99)

**Tip:** Vergroot de file descriptor limiet voor hoge client counts:
```bash
ulimit -n 10000  # in beide terminals (server + benchmark)
```

### Benchmark resultaten (Rust backend)

Getest met `rust-wsbench` → `rust-ws` op Apple M1 Pro:

#### Veel clients, lage rate (broadcast-bound)

| Clients | Rate/min | Throughput | P50 | P95 | P99 | Status |
|---------|----------|------------|-----|-----|-----|--------|
| 200 | 200 | 664 msg/s | 1ms | 4ms | 4ms | ✓ Excellent |
| 500 | 120 | 995 msg/s | 3ms | 5ms | 7ms | ✓ Excellent |
| 1000 | 60 | 990 msg/s | 9ms | 18ms | 91ms | ✓ Goed |
| 1500 | 30 | 744 msg/s | 16ms | 285ms | 1054ms | ⚠️ Grens |
| 2000 | 30 | 983 msg/s | 10s | 20s | 21s | ❌ Overbelast |

#### Weinig clients, hoge rate (throughput-bound)

| Clients | Rate/min | Throughput | P50 | P99 | Status |
|---------|----------|------------|-----|-----|--------|
| 10 | 30000 | 2968 msg/s | 0ms | 2ms | ✓ Excellent |
| 10 | 60000 | 4356 msg/s | 0ms | 2ms | ✓ Excellent |
| 10 | 75000 | overflow | 5.5s | 5.9s | ❌ Overbelast |

**Server limieten:**

| Bottleneck | Limiet | Scenario |
|------------|--------|----------|
| Max clients | 2000+ | Geen connection issues |
| Max inbound msg/s | ~5000 | Weinig clients, hoge rate |
| Max broadcasts/s | ~1M | Veel clients, lage rate |
| Sweet spot | 1000 clients @ 60/min | <100ms P99 |

### Rust vs Bun backend

Directe vergelijking met 500 clients @ 120 msg/min:

| Metric | Rust | Bun | Verschil |
|--------|------|-----|----------|
| Clients connected | 500/500 (100%) | 382/500 (76%) | Rust +31% |
| P50 latency | **3ms** | 8078ms | Rust 2700× sneller |
| P95 latency | **5ms** | 32224ms | Rust 6400× sneller |
| P99 latency | **7ms** | 39556ms | Rust 5600× sneller |
| Errors | 0 | 0 | - |

**Conclusie:** De Rust backend levert instant message delivery (<10ms) waar de Bun backend 8-40 seconden vertraging heeft onder dezelfde load. Dit is de reden dat Rust de standaard backend is.
