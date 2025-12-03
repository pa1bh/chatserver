import type { ServerWebSocket } from "bun";
import { createLogger } from "./logger";

type Client = {
  id: string;
  name: string;
  connectedAt: number;
};

type IncomingMessage =
  | { type: "chat"; text: string }
  | { type: "setName"; name: string }
  | { type: "status" }
  | { type: "listUsers" };

type OutgoingMessage =
  | { type: "chat"; from: string; text: string; at: number }
  | { type: "system"; text: string; at: number }
  | { type: "ackName"; name: string; at: number }
  | { type: "status"; uptimeSeconds: number; userCount: number; messagesSent: number }
  | { type: "listUsers"; users: Array<{ id: string; name: string }> }
  | { type: "error"; message: string };

const port = Number(process.env.WS_PORT ?? 3001);
const startedAt = Date.now();
let messagesSent = 0;

const { info, warn, error, target } = createLogger("ws");

const clients = new Map<string, ServerWebSocket>();
const clientInfo = new Map<string, Client>();
const clientIp = new Map<string, string>();

const send = (ws: ServerWebSocket, payload: OutgoingMessage) => {
  ws.send(JSON.stringify(payload));
};

const broadcast = (payload: OutgoingMessage, exceptId?: string) => {
  const data = JSON.stringify(payload);
  for (const [id, socket] of clients) {
    if (exceptId && id === exceptId) continue;
    socket.send(data);
  }
};

const parseIncoming = (raw: string): IncomingMessage | { type: "error"; message: string } => {
  try {
    const data = JSON.parse(raw);
    if (data?.type === "chat" && typeof data.text === "string") return { type: "chat", text: data.text };
    if (data?.type === "setName" && typeof data.name === "string") return { type: "setName", name: data.name };
    if (data?.type === "status") return { type: "status" };
    if (data?.type === "listUsers") return { type: "listUsers" };
  } catch {
    return { type: "error", message: "Bericht moet JSON zijn." };
  }
  return { type: "error", message: "Onbekend of onvolledig berichttype." };
};

const validateName = (name: string): string | null => {
  const trimmed = name.trim();
  if (trimmed.length < 2 || trimmed.length > 32) return "Naam moet tussen 2 en 32 tekens zijn.";
  return null;
};

const validateText = (text: string): string | null => {
  const trimmed = text.trim();
  if (!trimmed) return "Bericht mag niet leeg zijn.";
  if (trimmed.length > 500) return "Bericht is te lang (max 500 tekens).";
  return null;
};

const buildStatus = () => ({
  type: "status" as const,
  uptimeSeconds: Number((process.uptime()).toFixed(2)),
  userCount: clients.size,
  messagesSent,
});

const server = Bun.serve<{ id?: string; ip?: string }>({
  port,
  fetch(req, server) {
    const forwarded = req.headers.get("x-forwarded-for");
    const ip =
      forwarded?.split(",")[0].trim() ||
      server.requestIP(req)?.address ||
      "unknown";
    if (server.upgrade(req, { data: { ip } })) return undefined;
    return new Response("Upgrade Required", { status: 426 });
  },
  websocket: {
    open(ws) {
      const id = crypto.randomUUID();
      const name = `guest-${id.slice(0, 6)}`;
      ws.data = { id, ip: (ws.data as any)?.ip };
      clients.set(id, ws);
      if (ws.data.ip) clientIp.set(id, ws.data.ip);
      clientInfo.set(id, { id, name, connectedAt: Date.now() });
      info("Nieuwe gebruiker verbonden", { id, name, ip: ws.data.ip });
      send(ws, { type: "ackName", name, at: Date.now() });
      broadcast({ type: "system", text: `${name} heeft de chat betreden.`, at: Date.now() }, id);
    },
    message(ws, message) {
      const clientId = ws.data?.id;
      if (!clientId) return ws.close();
      const client = clientInfo.get(clientId);
      if (!client) return ws.close();

      const text = typeof message === "string" ? message : new TextDecoder().decode(message as ArrayBuffer);
      const parsed = parseIncoming(text);
      if (parsed.type === "error") {
        send(ws, parsed);
        return;
      }

      switch (parsed.type) {
        case "chat": {
          const validation = validateText(parsed.text);
          if (validation) return send(ws, { type: "error", message: validation });
          messagesSent += 1;
          const payload: OutgoingMessage = {
            type: "chat",
            from: client.name,
            text: parsed.text.trim(),
            at: Date.now(),
          };
          broadcast(payload);
          info("Bericht verzonden", { from: client.name, id: client.id, ip: ws.data?.ip });
          break;
        }
        case "setName": {
          const validation = validateName(parsed.name);
          if (validation) return send(ws, { type: "error", message: validation });
          const newName = parsed.name.trim();
          const oldName = client.name;
          client.name = newName;
          clientInfo.set(clientId, client);
          send(ws, { type: "ackName", name: newName, at: Date.now() });
          broadcast({ type: "system", text: `${oldName} heet nu ${newName}.`, at: Date.now() }, clientId);
          info("Gebruikersnaam gewijzigd", { oldName, newName, id: client.id, ip: ws.data?.ip });
          break;
        }
        case "status": {
          send(ws, buildStatus());
          break;
        }
        case "listUsers": {
          const users = Array.from(clientInfo.values()).map(({ id, name }) => ({ id, name }));
          send(ws, { type: "listUsers", users });
          break;
        }
        default:
          send(ws, { type: "error", message: "Onbekend berichttype." });
      }
    },
    close(ws) {
      const clientId = ws.data?.id;
      if (!clientId) return;
      const client = clientInfo.get(clientId);
      clients.delete(clientId);
      clientInfo.delete(clientId);
      const ip = clientIp.get(clientId);
      clientIp.delete(clientId);
      if (client) {
        broadcast({ type: "system", text: `${client.name} heeft de chat verlaten.`, at: Date.now() }, clientId);
        info("Gebruiker heeft de chat verlaten", { name: client.name, id: client.id, ip });
      }
    },
  },
});

info("WebSocket server gestart", {
  port,
  logTarget: target,
  uptimeSince: new Date(startedAt).toISOString(),
  upgrade: "ws",
});

export type WsServer = typeof server;
