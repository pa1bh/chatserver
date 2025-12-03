import express from "express";

const app = express();
const port = Number(process.env.PORT) || 3000;

app.get("/", (_req, res) => {
  res.send(`
    <main style="font-family: system-ui, -apple-system, sans-serif; max-width: 680px; margin: 64px auto; padding: 0 24px; line-height: 1.6;">
      <h1 style="margin: 0 0 16px; font-size: 32px;">Welkom bij Bunserve</h1>
      <p style="margin: 0 0 12px;">Deze homepage wordt geserveerd via Express, draaiend op de Bun runtime.</p>
      <p style="margin: 0;">Later voegen we meer routes en functionaliteit toe.</p>
    </main>
  `);
});

app.listen(port, () => {
  console.log(`Server draait op http://localhost:${port}`);
});
