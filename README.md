# bunserve

Een minimale Express-app die draait op de Bun runtime. Voor nu serveert hij alleen de homepage; later bouwen we dit verder uit.

## Installeren

```bash
bun install
```

## Starten

```bash
bun run index.ts
```

Voor hot reload tijdens ontwikkelen:

```bash
bun --watch index.ts
```

Deze setup is aangemaakt met `bun init` op Bun v1.0.0.

## Routes

- `/` - eenvoudige homepage.
- `/status` - JSON met runtime-informatie (uptime, Bun-versie, env, port, memory, aantal requests sinds start).
