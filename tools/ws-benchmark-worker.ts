/**
 * WebSocket Benchmark Worker
 * Spawned by ws-benchmark.ts - do not run directly
 */

declare var self: Worker;

interface WorkerConfig {
  workerId: number;
  url: string;
  rate: number; // messages per minute
  duration: number; // seconds
}

interface PendingMessage {
  id: string;
  sentAt: number;
}

// Random Dutch/English phrases for chat messages
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
  "Another test message here.",
  "Benchmark client reporting in.",
  "Message throughput test.",
  "Concurrent connection test.",
  "Real-time communication check.",
];

function randomPhrase(): string {
  const base = phrases[Math.floor(Math.random() * phrases.length)];
  // Add some randomness to make each message unique
  return base + Math.random().toString(36).substring(2, 8);
}

function randomInterval(baseMs: number): number {
  // Add ±30% randomness to the interval
  const variance = baseMs * 0.3;
  return baseMs + (Math.random() * 2 - 1) * variance;
}

self.onmessage = async (event: MessageEvent<WorkerConfig>) => {
  const config = event.data;
  const { workerId, url, rate, duration } = config;

  const stats = {
    workerId,
    connected: false,
    messagesSent: 0,
    messagesReceived: 0,
    errors: 0,
    latencies: [] as number[],
  };

  const pendingMessages = new Map<string, PendingMessage>();
  const baseIntervalMs = (60 * 1000) / rate;
  let ws: WebSocket | null = null;
  let running = true;
  let clientName = `bench-${workerId}`;

  const log = (msg: string) => {
    self.postMessage({ type: "log", workerId, message: msg });
  };

  const sendStats = () => {
    self.postMessage({ type: "stats", workerId, stats });
  };

  try {
    ws = new WebSocket(url);

    ws.onopen = () => {
      stats.connected = true;
      log("Connected");
      sendStats();

      // Set a unique name
      ws!.send(JSON.stringify({ type: "setName", name: clientName }));
    };

    ws.onmessage = (event) => {
      stats.messagesReceived++;

      try {
        const data = JSON.parse(event.data);

        // Track latency for our own chat messages (they get echoed back)
        if (data.type === "chat" && data.from === clientName) {
          const msgId = data.text.split("|")[0];
          const pending = pendingMessages.get(msgId);
          if (pending) {
            const latency = Date.now() - pending.sentAt;
            stats.latencies.push(latency);
            pendingMessages.delete(msgId);
          }
        }

        if (data.type === "ackName" && data.name) {
          clientName = data.name;
        }
      } catch {
        // Ignore parse errors for non-JSON messages
      }

      sendStats();
    };

    ws.onerror = (error) => {
      stats.errors++;
      log(`Error: ${error}`);
      sendStats();
    };

    ws.onclose = () => {
      stats.connected = false;
      log("Disconnected");
      sendStats();
    };

    // Wait for connection
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("Connection timeout")), 5000);
      const checkInterval = setInterval(() => {
        if (ws?.readyState === WebSocket.OPEN) {
          clearTimeout(timeout);
          clearInterval(checkInterval);
          resolve();
        } else if (ws?.readyState === WebSocket.CLOSED) {
          clearTimeout(timeout);
          clearInterval(checkInterval);
          reject(new Error("Connection closed"));
        }
      }, 50);
    });

    // Send messages at the configured rate
    const endTime = Date.now() + duration * 1000;

    while (running && Date.now() < endTime) {
      if (ws.readyState !== WebSocket.OPEN) {
        break;
      }

      // Create message with tracking ID
      const msgId = `${workerId}-${stats.messagesSent}`;
      const text = `${msgId}|${randomPhrase()}`;

      pendingMessages.set(msgId, {
        id: msgId,
        sentAt: Date.now(),
      });

      ws.send(JSON.stringify({ type: "chat", text }));
      stats.messagesSent++;
      sendStats();

      // Wait with random interval
      const waitMs = randomInterval(baseIntervalMs);
      await new Promise((resolve) => setTimeout(resolve, waitMs));
    }
  } catch (error) {
    stats.errors++;
    log(`Fatal error: ${error}`);
  } finally {
    running = false;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.close();
    }
    sendStats();
    self.postMessage({ type: "done", workerId });
  }
};
