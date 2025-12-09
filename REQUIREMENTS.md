# Project Requirements

## Goal and Scope
This project is a chat server/client based on WebSockets.

There are two entrypoints (processes):
- A web server to serve the client code to the browser
- A WebSocket backend to handle the chat messages

Both may reside in the same project but must be startable independently, allowing the two responsibilities to be deployed on different servers/containers.

## Target Audience and Use Cases
- Primary users: Visitors to the chat server who want to chat with each other
- Main use cases / user stories:
  * A visitor opens the site in their browser and loads the (JS) frontend
  * The user can set a nickname and immediately see all live chat messages
  * There is a chat window with a message input field at the bottom
  * Slash commands (`/`) can be used for system commands

## Functional Requirements

### Routes/Endpoints
- `/` - Homepage with chat frontend
- `/status` - Server status JSON

### Frontend Commands
- `/name <username>` - Change username
- `/status` - Request server status
- `/users` - List connected users
- `/ping [token]` - Measure roundtrip latency
- `/ai <question>` - Ask AI a question (requires configuration)

### WebSocket Protocol

#### Client → Server
- `{ type: "chat", text }` - Send message
- `{ type: "setName", name }` - Change username
- `{ type: "status" }` - Request server status
- `{ type: "listUsers" }` - Request user list
- `{ type: "ping", token? }` - Ping with optional token
- `{ type: "ai", prompt }` - Ask AI a question

#### Server → Client
- `chat { from, text, at }` - Chat message
- `system { text, at }` - Join/leave/rename events
- `ackName { name, at }` - Name change confirmation
- `status { uptimeSeconds, userCount, messagesSent, messagesPerSecond, memoryMb }`
- `listUsers { users: [{ id, name, ip }] }`
- `pong { token?, at }` - Response to ping
- `ai { from, prompt, response, at }` - AI response broadcast
- `error { message }` - Error message

### Backend Implementations
- **Rust (recommended)**: `rust-ws/` - Axum/Tokio based, high performance
- **Bun/TypeScript (deprecated)**: `ws-server.ts` - For testing only

### Native Clients
- `rust-client/` - CLI chat client with command history
- `rust-gui/` - GUI chat client using egui
- `rust-wsmonitor/` - Health check CLI tool for scripts
- `rust-wsbench/` - Load testing / benchmark tool

### AI Integration
- OpenRouter integration for AI-powered Q&A
- Rate limiting per user
- Responses broadcast to all connected users

## Non-Functional Requirements

### Performance
- No specific requirements at this stage
- Benchmark results available in README.md

### Availability/Uptime
- No specific requirements at this stage

### Scalability
- WebSocket and web server processes are separated
- Supports containerized deployment (Docker)

### Security
- No authentication or banning in this phase
- Rate limiting on AI requests only
- Input validation on chat messages

### Observability
Logging must include:
- New user connections
- User X sends message
- User X disconnects
- AI requests and responses

Logging destination configurable via:
- `LOG_TARGET` environment variable (`stdout` or `file`)
- `--log=stdout` or `--log=file:path` CLI argument
- `RUST_LOG` for Rust backend log level

## Configuration

### Environment Variables
| Variable | Default | Purpose |
|----------|---------|---------|
| `PORT` | 3000 | HTTP server port |
| `HOST` | 0.0.0.0 | HTTP server bind address |
| `WS_PORT` | 3001 | WebSocket server port |
| `WS_HOST` | - | Override WebSocket hostname |
| `WS_URL` | - | Full WebSocket URL override |
| `LOG_TARGET` | stdout | `stdout` or `file` |
| `LOG_FILE` | - | Log file path |
| `RUST_LOG` | - | Rust log level (`info`, `debug`) |
| `OPENROUTER_API_KEY` | - | OpenRouter API key for AI |
| `AI_ENABLED` | false | Enable/disable AI feature |
| `AI_MODEL` | openai/gpt-4o | AI model to use |
| `AI_RATE_LIMIT` | 5 | Max AI requests per user per minute |
| `RATE_LIMIT_ENABLED` | false | Enable chat rate limiting |
| `RATE_LIMIT_MSG_PER_MIN` | 60 | Max chat messages per user per minute |

## Quality & Testing

### Test Strategy
- No automated test suite configured yet
- Manual testing via:
  - `rust-wsmonitor` for health checks
  - `rust-wsbench` for load testing
  - `websocat` for protocol testing

### Coverage Goals
- None defined at this stage

## Deployment & Operations

### Deployment
- Docker support for Rust backend (`rust-ws/Dockerfile`)
- Multi-stage build (~15MB image with Alpine Linux)

### Monitoring/Alerts
- Health checks via `rust-wsmonitor`
- `/status` endpoint for HTTP server status

### Backups and Recovery
- No persistent data storage; no backup requirements

## Open Questions / Assumptions
- Ensure agreements in AGENTS.md and CLAUDE.md are honored
