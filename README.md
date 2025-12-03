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
- `WS_PORT` (default `3001`): WebSocket server.
- `WS_URL`: optioneel volledig WebSocket-adres dat in de frontend wordt geïnjecteerd (anders `ws://localhost:WS_PORT`).
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
