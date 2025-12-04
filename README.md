# bunserve

Chatserver op Bun met een Express frontend en een WebSocket backend.

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

WebSocket backend:
```bash
bun run ws-server.ts
# of: bun run start:ws
```

Tijdens ontwikkeling:
```bash
bun run dev:http
bun run dev:ws
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
  - `status` `{ uptimeSeconds, userCount, messagesSent }`
  - `listUsers` `{ users: [{ id, name }] }`
  - `error` `{ message }`

## Frontend commands
- `/name nieuwe_naam` — wijzig gebruikersnaam.
- `/status` — vraag serverstatus op.
- `/users` — lijst huidige gebruikers.

## Logging
Logging gaat naar stdout tenzij `LOG_TARGET=file` of `--log=file:pad` is gezet. Backend logt join/leave/bericht events; HTTP logt startinfo.

## Alternatieve WS backend (Rust)

Drop-in vervanging voor de Bun/TS WebSocket server, geschreven in Rust met Axum en Tokio.

- **Locatie:** `rust-ws/`
- **Vereisten:** Rust + Cargo
- **Protocol:** identiek aan de Bun/TS backend — de frontend werkt ongewijzigd

### Bouwen en starten

```bash
cd rust-ws
cargo build --release    # optioneel: release build
cargo run                # start op WS_PORT (default 3001)
```

Met debug logging:
```bash
RUST_LOG=debug cargo run
```

### Configuratie

De Rust backend leest dezelfde `WS_PORT` environment variable als de TS versie.

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
