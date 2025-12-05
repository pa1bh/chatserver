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
- Outbound (server → client):
  - `chat` `{ from, text, at }`
  - `system` `{ text, at }`
  - `ackName` `{ name, at }`
  - `status` `{ uptimeSeconds, userCount, messagesSent, messagesPerSecond, memoryMb }` ¹
  - `listUsers` `{ users: [{ id, name, ip }] }` ¹
  - `error` `{ message }`

¹ Rust backend only: `messagesPerSecond`, `memoryMb`, en `ip` velden

## Frontend commands
- `/name nieuwe_naam` — wijzig gebruikersnaam.
- `/status` — vraag serverstatus op.
- `/users` — lijst huidige gebruikers.

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

Stress test tool om de Bun/TS en Rust backends te vergelijken. Gebruikt Bun Worker threads voor realistische concurrent clients.

Er zijn twee versies:
- `ws-benchmark.ts` — Worker threads (realistischer, maar max ~100 clients door Bun bug)
- `ws-benchmark-async.ts` — Async in één process (stabieler voor hoge loads)

### Gebruik

```bash
# Worker versie (tot ~100 clients)
bun run tools/ws-benchmark.ts --clients=50 --rate=120 --duration=60

# Async versie (voor 100+ clients)
bun run tools/ws-benchmark-async.ts --clients=200 --rate=600 --duration=60

# Tegen specifieke URL
bun run tools/ws-benchmark.ts --url=ws://192.168.0.80:3001

# Quiet mode (alleen resultaten)
bun run tools/ws-benchmark.ts --quiet
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

#### Voorbeeld test
```bash
bun run tools/ws-benchmark-async.ts --clients=200 --rate=600 --duration=60
```

rust backend
```bash
═══════════════════════════════════════
Results
═══════════════════════════════════════
Clients connected:  200/200
Messages sent:      115516
Messages received:  23152470
Errors:             0
Throughput:         1925.3 msg/s

Latency (ms):
  Average:  2.67
  P50:      2.00
  P95:      5.00
  P99:      7.00
═══════════════════════════════════════
```

Bun backend
```bash
═══════════════════════════════════════
Results
═══════════════════════════════════════
Clients connected:  200/200
Messages sent:      115568
Messages received:  15955826
Errors:             0
Throughput:         1926.1 msg/s

Latency (ms):
  Average:  7594.50
  P50:      5248.00
  P95:      22254.00
  P99:      27827.00
═══════════════════════════════════════
```
