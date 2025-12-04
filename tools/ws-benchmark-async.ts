#!/usr/bin/env bun
/**
 * WebSocket Benchmark Tool (Async version - no Workers)
 *
 * More stable for high client counts. All connections run in a single process.
 *
 * Usage:
 *   bun run tools/ws-benchmark-async.ts [options]
 *
 * Options:
 *   --url=<ws://...>     WebSocket URL (default: ws://127.0.0.1:3001)
 *   --clients=<n>        Number of concurrent clients (default: 10)
 *   --rate=<n>           Messages per minute per client (default: 60)
 *   --duration=<s>       Test duration in seconds (default: 30)
 *   --quiet              Only show summary, no per-message logs
 */

function parseArgs(argv: string[]): Record<string, string | boolean> {
  const result: Record<string, string | boolean> = {};
  for (const arg of argv) {
    if (arg.startsWith("--")) {
      const [key, value] = arg.slice(2).split("=");
      result[key] = value ?? true;
    }
  }
  return result;
}

const args = parseArgs(Bun.argv.slice(2));

const config = {
  url: (args.url as string) || "ws://127.0.0.1:3001",
  clients: parseInt((args.clients as string) || "10", 10),
  rate: parseInt((args.rate as string) || "60", 10),
  duration: parseInt((args.duration as string) || "30", 10),
  quiet: args.quiet === true,
};

// Random phrases for chat messages
const phrases = [
  "Hallo, hoe gaat het?",
  "De zon schijnt vandaag!",
  "Wat een mooi weer.",
  "Ik ben aan het testen.",
  "Dit is een benchmark bericht.",
  "Hello from the benchmark tool!",
  "Testing WebSocket performance.",
  "Random message number ",
  "How fast can we go?",
  "Stress testing in progress...",
  "The quick brown fox jumps over the lazy dog.",
  "Lorem ipsum dolor sit amet.",
  "WebSocket verbinding werkt prima.",
  "Server response time check.",
  "Latency measurement ongoing.",
];

function randomPhrase(): string {
  return phrases[Math.floor(Math.random() * phrases.length)] + Math.random().toString(36).substring(2, 8);
}

function randomInterval(baseMs: number): number {
  const variance = baseMs * 0.3;
  return baseMs + (Math.random() * 2 - 1) * variance;
}

interface ClientState {
  id: number;
  ws: WebSocket | null;
  connected: boolean;
  name: string;
  messagesSent: number;
  messagesReceived: number;
  errors: number;
  latencies: number[];
  pendingMessages: Map<string, number>;
}

const clients: ClientState[] = [];
const baseIntervalMs = (60 * 1000) / config.rate;

console.log(`
WebSocket Benchmark (Async)
═══════════════════════════════════════
URL:        ${config.url}
Clients:    ${config.clients}
Rate:       ${config.rate} msg/min/client
Duration:   ${config.duration}s
═══════════════════════════════════════
`);

// Create all client states
for (let i = 0; i < config.clients; i++) {
  clients.push({
    id: i,
    ws: null,
    connected: false,
    name: `bench-${i}`,
    messagesSent: 0,
    messagesReceived: 0,
    errors: 0,
    latencies: [],
    pendingMessages: new Map(),
  });
}

const log = (clientId: number, msg: string) => {
  if (!config.quiet) {
    console.log(`[Client ${clientId}] ${msg}`);
  }
};

// Connect a single client
async function connectClient(client: ClientState): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Connection timeout")), 5000);

    try {
      const ws = new WebSocket(config.url);
      client.ws = ws;

      ws.onopen = () => {
        clearTimeout(timeout);
        client.connected = true;
        log(client.id, "Connected");
        ws.send(JSON.stringify({ type: "setName", name: client.name }));
        resolve();
      };

      ws.onmessage = (event) => {
        client.messagesReceived++;
        try {
          const data = JSON.parse(event.data as string);

          if (data.type === "ackName" && data.name) {
            client.name = data.name;
          }

          // Track latency for echoed messages
          if (data.type === "chat" && data.from === client.name && data.text) {
            const msgId = data.text.split("|")[0];
            const sentAt = client.pendingMessages.get(msgId);
            if (sentAt) {
              client.latencies.push(Date.now() - sentAt);
              client.pendingMessages.delete(msgId);
            }
          }
        } catch {
          // Ignore parse errors
        }
      };

      ws.onerror = () => {
        client.errors++;
      };

      ws.onclose = () => {
        client.connected = false;
      };
    } catch (err) {
      clearTimeout(timeout);
      client.errors++;
      reject(err);
    }
  });
}

// Send messages for a single client
async function runClient(client: ClientState, endTime: number): Promise<void> {
  while (Date.now() < endTime && client.connected && client.ws?.readyState === WebSocket.OPEN) {
    const msgId = `${client.id}-${client.messagesSent}`;
    const text = `${msgId}|${randomPhrase()}`;

    client.pendingMessages.set(msgId, Date.now());
    client.ws.send(JSON.stringify({ type: "chat", text }));
    client.messagesSent++;

    await Bun.sleep(randomInterval(baseIntervalMs));
  }
}

// Connect all clients in batches
const BATCH_SIZE = 50;
const BATCH_DELAY_MS = 100;

console.log("Connecting clients...");
for (let i = 0; i < config.clients; i += BATCH_SIZE) {
  const batch = clients.slice(i, Math.min(i + BATCH_SIZE, config.clients));
  await Promise.all(
    batch.map((client) =>
      connectClient(client).catch((err) => {
        client.errors++;
        log(client.id, `Connection failed: ${err.message}`);
      })
    )
  );
  if (i + BATCH_SIZE < config.clients) {
    await Bun.sleep(BATCH_DELAY_MS);
  }
}

const connectedCount = clients.filter((c) => c.connected).length;
console.log(`Connected: ${connectedCount}/${config.clients}\n`);

// Start all clients sending messages
const startTime = Date.now();
const endTime = startTime + config.duration * 1000;

// Progress indicator
const progressInterval = setInterval(() => {
  const elapsed = Math.floor((Date.now() - startTime) / 1000);
  const connected = clients.filter((c) => c.connected).length;
  const totalSent = clients.reduce((sum, c) => sum + c.messagesSent, 0);
  const totalRecv = clients.reduce((sum, c) => sum + c.messagesReceived, 0);

  console.log(
    `[${elapsed}s/${config.duration}s] Connected: ${connected}/${config.clients} | Sent: ${totalSent} | Recv: ${totalRecv}`
  );
}, 1000);

// Run all clients concurrently
await Promise.all(clients.filter((c) => c.connected).map((client) => runClient(client, endTime)));

clearInterval(progressInterval);

// Close all connections
clients.forEach((c) => {
  if (c.ws && c.ws.readyState === WebSocket.OPEN) {
    c.ws.close();
  }
});

// Wait a moment for final messages
await Bun.sleep(500);

// Calculate stats
const totalSent = clients.reduce((sum, c) => sum + c.messagesSent, 0);
const totalRecv = clients.reduce((sum, c) => sum + c.messagesReceived, 0);
const totalErrors = clients.reduce((sum, c) => sum + c.errors, 0);
const allLatencies = clients.flatMap((c) => c.latencies);
const avgLatency = allLatencies.length > 0 ? allLatencies.reduce((a, b) => a + b, 0) / allLatencies.length : 0;

function percentile(arr: number[], p: number): number {
  if (arr.length === 0) return 0;
  const sorted = [...arr].sort((a, b) => a - b);
  const idx = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, idx)];
}

const p50 = percentile(allLatencies, 50);
const p95 = percentile(allLatencies, 95);
const p99 = percentile(allLatencies, 99);

console.log(`
═══════════════════════════════════════
Results
═══════════════════════════════════════
Clients connected:  ${connectedCount}/${config.clients}
Messages sent:      ${totalSent}
Messages received:  ${totalRecv}
Errors:             ${totalErrors}
Throughput:         ${(totalSent / config.duration).toFixed(1)} msg/s

Latency (ms):
  Average:  ${avgLatency.toFixed(2)}
  P50:      ${p50.toFixed(2)}
  P95:      ${p95.toFixed(2)}
  P99:      ${p99.toFixed(2)}
═══════════════════════════════════════
`);
