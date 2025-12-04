# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Install dependencies
bun install

# HTTP server (frontend + status endpoint)
bun run start:http          # or: bun run index.ts

# WebSocket server (TypeScript/Bun)
bun run start:ws            # or: bun run ws-server.ts

# Development with auto-reload (run in separate terminals)
bun run dev:http
bun run dev:ws

# Rust WebSocket backend (alternative)
cd rust-ws && cargo run
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

CLI logging: `--log=stdout` or `--log=file:server.log`

## Architecture

The project is a chat server with two separate processes:

1. **HTTP Server** (`index.ts`) - Express on Bun serving the chat frontend at `/` and status JSON at `/status`. Injects WebSocket URL dynamically based on request headers.

2. **WebSocket Server** (`ws-server.ts`) - Bun native WebSocket API handling real-time chat. Manages client connections, broadcasts messages, tracks users.

3. **Rust WebSocket Backend** (`rust-ws/`) - Drop-in replacement using Axum/Tokio. Same protocol as TypeScript version.

### WebSocket Protocol

Inbound (client → server):
- `{ type: "chat", text }` - Send message
- `{ type: "setName", name }` - Change username
- `{ type: "status" }` - Request server status
- `{ type: "listUsers" }` - Request user list

Outbound (server → client):
- `chat { from, text, at }` - Chat message
- `system { text, at }` - Join/leave/rename events
- `ackName { name, at }` - Name change confirmation
- `status { uptimeSeconds, userCount, messagesSent }`
- `listUsers { users: [{ id, name }] }`
- `error { message }`

### Frontend Commands

- `/name <username>` - Change username
- `/status` - Get server status
- `/users` - List connected users

## Code Style

- TypeScript with strict mode, ES modules
- 2-space indentation
- Prefer early returns
- Environment config via `process.env` with safe defaults
- Conventional commits: `feat:`, `fix:`, `chore:`, etc.

## Testing

No test suite configured yet. Recommended: `vitest` or `bun:test`. Test files should mirror `src/` structure (e.g., `tests/health.test.ts`).
