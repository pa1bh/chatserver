#!/usr/bin/env bun
/**
 * WebSocket Benchmark Tool
 *
 * Usage:
 *   bun run tools/ws-benchmark.ts [options]
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

interface WorkerStats {
  workerId: number;
  connected: boolean;
  messagesSent: number;
  messagesReceived: number;
  errors: number;
  latencies: number[];
}

interface WorkerMessage {
  type: "stats" | "log" | "done";
  workerId: number;
  stats?: WorkerStats;
  message?: string;
}

const stats: WorkerStats[] = [];
const workers: Worker[] = [];

console.log(`
WebSocket Benchmark
═══════════════════════════════════════
URL:        ${config.url}
Clients:    ${config.clients}
Rate:       ${config.rate} msg/min/client
Duration:   ${config.duration}s
═══════════════════════════════════════
`);

// Spawn workers in batches to avoid Bun runtime crash
const BATCH_SIZE = 20;
const BATCH_DELAY_MS = 50;

async function spawnWorkers() {
  for (let i = 0; i < config.clients; i++) {
    const worker = new Worker(new URL("./ws-benchmark-worker.ts", import.meta.url).href);

    worker.postMessage({
      workerId: i,
      url: config.url,
      rate: config.rate,
      duration: config.duration,
    });

    worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
      const msg = event.data;

      if (msg.type === "log" && !config.quiet) {
        console.log(`[Worker ${msg.workerId}] ${msg.message}`);
      } else if (msg.type === "stats" && msg.stats) {
        stats[msg.workerId] = msg.stats;
      } else if (msg.type === "done") {
        worker.terminate();
      }
    };

    workers.push(worker);

    // Stagger worker spawns to avoid overwhelming Bun's runtime
    if ((i + 1) % BATCH_SIZE === 0 && i < config.clients - 1) {
      await Bun.sleep(BATCH_DELAY_MS);
    }
  }
}

await spawnWorkers();

// Progress indicator
const startTime = Date.now();
let lastLine = "";
const progressInterval = setInterval(() => {
  const elapsed = Math.floor((Date.now() - startTime) / 1000);
  const connected = stats.filter((s) => s?.connected).length;
  const totalSent = stats.reduce((sum, s) => sum + (s?.messagesSent || 0), 0);
  const totalRecv = stats.reduce((sum, s) => sum + (s?.messagesReceived || 0), 0);

  const line = `[${elapsed}s/${config.duration}s] Connected: ${connected}/${config.clients} | Sent: ${totalSent} | Recv: ${totalRecv}`;
  if (line !== lastLine) {
    console.log(line);
    lastLine = line;
  }
}, 1000);

// Wait for duration + grace period
await Bun.sleep((config.duration + 2) * 1000);
clearInterval(progressInterval);

// Terminate any remaining workers
workers.forEach((w) => w.terminate());

// Calculate final stats
const totalSent = stats.reduce((sum, s) => sum + (s?.messagesSent || 0), 0);
const totalRecv = stats.reduce((sum, s) => sum + (s?.messagesReceived || 0), 0);
const totalErrors = stats.reduce((sum, s) => sum + (s?.errors || 0), 0);
const allLatencies = stats.flatMap((s) => s?.latencies || []);
const avgLatency = allLatencies.length > 0 ? allLatencies.reduce((a, b) => a + b, 0) / allLatencies.length : 0;
const p50 = percentile(allLatencies, 50);
const p95 = percentile(allLatencies, 95);
const p99 = percentile(allLatencies, 99);

console.log(`\n
═══════════════════════════════════════
Results
═══════════════════════════════════════
Clients connected:  ${stats.filter((s) => s?.connected).length}/${config.clients}
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

function percentile(arr: number[], p: number): number {
  if (arr.length === 0) return 0;
  const sorted = [...arr].sort((a, b) => a - b);
  const idx = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, idx)];
}
