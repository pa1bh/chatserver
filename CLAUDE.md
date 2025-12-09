# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Install frontend dependencies
bun install

# HTTP server (frontend + status endpoint)
bun run start:http          # or: bun run index.ts

# WebSocket server (Rust - recommended)
cd rust-ws && cargo run --release

# Development with auto-reload
bun run dev:http            # frontend with auto-reload
cd rust-ws && cargo run     # Rust backend (debug mode)

# Bun WebSocket backend (deprecated, for testing only)
bun run start:ws            # or: bun run ws-server.ts
```

## Environment Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `PORT` | 3000 | HTTP server port |
| `HOST` | 0.0.0.0 | HTTP server bind address |
| `WS_PORT` | 3001 | WebSocket server port |
| `WS_HOST` | - | Override WebSocket hostname (for reverse proxy) |
| `WS_URL` | - | Full WebSocket URL override |
| `LOG_TARGET` | stdout | `stdout` or `file` |
| `LOG_FILE` | - | Log file path when `LOG_TARGET=file` |
| `RUST_LOG` | - | Rust logging level (`info`, `debug`) |
| `RATE_LIMIT_ENABLED` | false | Enable chat rate limiting |
| `RATE_LIMIT_MSG_PER_MIN` | 60 | Max chat messages per user per minute |

CLI logging: `--log=stdout` or `--log=file:server.log`

## Architecture

The project is a chat server with two separate processes:

1. **HTTP Server** (`index.ts`) - Express on Bun serving the chat frontend at `/` and status JSON at `/status`. Injects WebSocket URL dynamically based on request headers.

2. **WebSocket Server** (`rust-ws/`) - Rust backend using Axum/Tokio. Handles real-time chat, client connections, broadcasts, user tracking. This is the recommended backend for production.

3. **Bun WebSocket Backend** (`ws-server.ts`) - Deprecated TypeScript version. Same protocol but lower performance.

### Native Clients

- `rust-client/` - CLI chat client with command history
- `rust-gui/` - GUI chat client using egui
- `rust-wsmonitor/` - Health check CLI tool for scripts

### WebSocket Protocol

Inbound (client → server):
- `{ type: "chat", text }` - Send message
- `{ type: "setName", name }` - Change username
- `{ type: "status" }` - Request server status
- `{ type: "listUsers" }` - Request user list
- `{ type: "ping", token? }` - Ping with optional token for validation

Outbound (server → client):
- `chat { from, text, at }` - Chat message
- `system { text, at }` - Join/leave/rename events
- `ackName { name, at }` - Name change confirmation
- `status { uptimeSeconds, userCount, messagesSent, messagesPerSecond, memoryMb }`
- `listUsers { users: [{ id, name, ip }] }`
- `pong { token?, at }` - Response to ping
- `error { message }`

### Frontend Commands

- `/name <username>` - Change username
- `/status` - Get server status
- `/users` - List connected users
- `/ping [token]` - Measure roundtrip time

## Code Style

- TypeScript with strict mode, ES modules (frontend)
- Rust with standard formatting (backend)
- 2-space indentation (TS), 4-space (Rust)
- Prefer early returns
- Environment config via `process.env` / `std::env` with safe defaults
- Conventional commits: `feat:`, `fix:`, `chore:`, etc.

## Testing

No test suite configured yet. For manual testing:
- `rust-wsmonitor` for health checks
- `rust-wsbench` for load testing
