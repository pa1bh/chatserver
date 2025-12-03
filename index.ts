import express from "express";
import os from "os";
import { createLogger } from "./logger";

const app = express();
const port = Number(process.env.PORT) || 3000;
const host = process.env.HOST ?? "0.0.0.0";
const wsPort = Number(process.env.WS_PORT || 3001);
const wsUrl = process.env.WS_URL ?? `ws://localhost:${wsPort}`;
const startedAt = new Date();
let requestCount = 0;
const { info, target } = createLogger("http");

const toMB = (bytes: number) => Number((bytes / 1024 / 1024).toFixed(2));
const getLocalUrls = (p: number) => {
  const nets = os.networkInterfaces();
  const urls = new Set<string>();
  urls.add(`http://localhost:${p}`);
  for (const net of Object.values(nets)) {
    if (!net) continue;
    for (const { address, family, internal } of net) {
      if (internal || family !== "IPv4") continue;
      urls.add(`http://${address}:${p}`);
    }
  }
  return Array.from(urls);
};
const indexTemplate = await Bun.file("public/index.html").text();

app.use((req, _res, next) => {
  requestCount += 1;
  next();
});

app.get("/", (_req, res) => {
  const page = indexTemplate.replace(/__WS_URL__/g, wsUrl);
  res.type("html").send(page);
});

app.use(express.static("public"));

app.get("/status", (_req, res) => {
  const memory = process.memoryUsage();

  res.json({
    status: "ok",
    runtime: "bun",
    bunVersion: Bun.version,
    nodeEnv: process.env.NODE_ENV ?? "development",
    port,
    wsUrl,
    startedAt: startedAt.toISOString(),
    uptimeSeconds: Number(process.uptime().toFixed(2)),
    requestsHandled: requestCount,
    memoryMB: {
      rss: toMB(memory.rss),
      heapUsed: toMB(memory.heapUsed),
    },
  });
});

app.listen(port, host, () => {
  const urls = getLocalUrls(port);
  info("HTTP server gestart", { port, host, wsUrl, logTarget: target, urls });
});
