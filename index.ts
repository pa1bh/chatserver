import express from "express";

const app = express();
const port = Number(process.env.PORT) || 3000;
const startedAt = new Date();
let requestCount = 0;

const toMB = (bytes: number) => Number((bytes / 1024 / 1024).toFixed(2));

app.use((_req, _res, next) => {
  requestCount += 1;
  next();
});

app.get("/", (_req, res) => {
  res.send(`
    <main style="font-family: system-ui, -apple-system, sans-serif; max-width: 680px; margin: 64px auto; padding: 0 24px; line-height: 1.6;">
      <h1 style="margin: 0 0 16px; font-size: 32px;">Welkom bij Bunserve</h1>
      <p style="margin: 0 0 12px;">Deze homepage wordt geserveerd via Express, draaiend op de Bun runtime.</p>
      <p style="margin: 0;">Later voegen we meer routes en functionaliteit toe.</p>
      <p style="margin: 24px 0 0;"><a href="/status" style="color: #0f6ad8;">Bekijk serverstatus</a></p>
    </main>
  `);
});

app.get("/status", (_req, res) => {
  const memory = process.memoryUsage();

  res.json({
    status: "ok",
    runtime: "bun",
    bunVersion: Bun.version,
    nodeEnv: process.env.NODE_ENV ?? "development",
    port,
    startedAt: startedAt.toISOString(),
    uptimeSeconds: Number(process.uptime().toFixed(2)),
    requestsHandled: requestCount,
    memoryMB: {
      rss: toMB(memory.rss),
      heapUsed: toMB(memory.heapUsed),
    },
  });
});

app.listen(port, () => {
  console.log(`Server draait op http://localhost:${port}`);
});
