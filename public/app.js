const qs = (sel) => document.querySelector(sel);

const metaWs = qs('meta[name="ws-url"]');
const WS_URL = metaWs?.content || `ws://${location.hostname}:3001`;

const messagesEl = qs("#messages");
const form = qs("#chatForm");
const input = qs("#chatInput");
const nicknameInput = qs("#nickname");
const saveNameBtn = qs("#saveName");
const connectionStatus = qs("#connectionStatus");
const nameBadge = qs("#nameBadge");
const currentNameEl = qs("#currentName");
const nameModal = qs("#nameModal");
const closeNameBtn = qs("#closeName");
const composerName = qs("#composerName");
const composerNameText = qs("#composerNameText");

let socket;
let currentName = "";
let reconnectTimeout;
let pendingPings = new Map(); // token -> timestamp

const generateToken = () => {
  if (crypto.randomUUID) return crypto.randomUUID();
  return Array.from(crypto.getRandomValues(new Uint8Array(16)))
    .map(b => b.toString(16).padStart(2, "0"))
    .join("");
};

const getInitials = (name) => {
  if (!name) return "?";
  const parts = name.trim().split(/[-_\s]+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return name.slice(0, 2).toUpperCase();
};

const formatUptime = (seconds) => {
  if (seconds < 60) return `${seconds} sec`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} uur`;
  return `${Math.floor(seconds / 86400)} dagen`;
};

const setStatus = (text, tone = "info") => {
  connectionStatus.textContent = text;
  connectionStatus.style.background = tone === "error" ? "#fff1f2" : tone === "warn" ? "#fef9c3" : "#eef2ff";
  connectionStatus.style.color = tone === "error" ? "#9f1239" : tone === "warn" ? "#854d0e" : "#312e81";
};

const scrollToBottom = () => {
  requestAnimationFrame(() => {
    messagesEl.scrollTop = messagesEl.scrollHeight;
  });
};

// Simple markdown parser for AI responses
const parseMarkdown = (text) => {
  // Escape HTML first
  const escape = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  // Process code blocks first (```code```)
  const codeBlocks = [];
  text = text.replace(/```(\w*)\n?([\s\S]*?)```/g, (_, lang, code) => {
    codeBlocks.push(`<pre><code>${escape(code.trim())}</code></pre>`);
    return `\x00CODEBLOCK${codeBlocks.length - 1}\x00`;
  });

  // Process inline code (`code`)
  const inlineCode = [];
  text = text.replace(/`([^`]+)`/g, (_, code) => {
    inlineCode.push(`<code>${escape(code)}</code>`);
    return `\x00INLINE${inlineCode.length - 1}\x00`;
  });

  // Escape remaining HTML
  text = escape(text);

  // Bold (**text** or __text__)
  text = text.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  text = text.replace(/__(.+?)__/g, "<strong>$1</strong>");

  // Italic (*text* or _text_)
  text = text.replace(/\*([^*]+)\*/g, "<em>$1</em>");
  text = text.replace(/_([^_]+)_/g, "<em>$1</em>");

  // Headers (### text)
  text = text.replace(/^### (.+)$/gm, "<h4>$1</h4>");
  text = text.replace(/^## (.+)$/gm, "<h3>$1</h3>");
  text = text.replace(/^# (.+)$/gm, "<h3>$1</h3>");

  // Lists (- item or * item)
  text = text.replace(/^[-*] (.+)$/gm, "<li>$1</li>");
  text = text.replace(/(<li>.*<\/li>\n?)+/g, "<ul>$&</ul>");

  // Numbered lists (1. item)
  text = text.replace(/^\d+\. (.+)$/gm, "<li>$1</li>");

  // Line breaks
  text = text.replace(/\n/g, "<br>");

  // Restore code blocks and inline code
  text = text.replace(/\x00CODEBLOCK(\d+)\x00/g, (_, i) => codeBlocks[i]);
  text = text.replace(/\x00INLINE(\d+)\x00/g, (_, i) => inlineCode[i]);

  return text;
};

const appendMessage = (type, text, meta = "", isMine = false, useMarkdown = false) => {
  const item = document.createElement("div");
  item.className = `msg ${type} ${isMine ? "mine" : ""}`;
  const metaEl = document.createElement("div");
  metaEl.className = "meta";
  metaEl.textContent = meta;
  const body = document.createElement("div");
  body.className = "body";
  if (useMarkdown) {
    body.innerHTML = parseMarkdown(text);
  } else {
    body.textContent = text;
  }
  item.appendChild(metaEl);
  item.appendChild(body);
  messagesEl.appendChild(item);
  scrollToBottom();
};

const appendUserList = (users) => {
  const item = document.createElement("div");
  item.className = "msg system userlist";
  const metaEl = document.createElement("div");
  metaEl.className = "meta";
  metaEl.textContent = "server";
  const body = document.createElement("div");
  body.className = "body";

  const header = document.createElement("div");
  header.className = "userlist-header";
  header.textContent = `Gebruikers (${users.length})`;
  body.appendChild(header);

  const table = document.createElement("table");
  table.className = "userlist-table";

  users.forEach((u) => {
    const row = document.createElement("tr");

    const idCell = document.createElement("td");
    idCell.className = "userlist-id";
    idCell.textContent = u.id;
    row.appendChild(idCell);

    const nameCell = document.createElement("td");
    nameCell.className = "userlist-name";
    nameCell.textContent = u.name;
    if (u.name === currentName) nameCell.classList.add("userlist-me");
    row.appendChild(nameCell);

    if (u.ip) {
      const ipCell = document.createElement("td");
      ipCell.className = "userlist-ip";
      ipCell.textContent = u.ip;
      row.appendChild(ipCell);
    }

    table.appendChild(row);
  });

  body.appendChild(table);
  item.appendChild(metaEl);
  item.appendChild(body);
  messagesEl.appendChild(item);
  scrollToBottom();
};

const appendStatus = (payload) => {
  const item = document.createElement("div");
  item.className = "msg system status-panel";
  const metaEl = document.createElement("div");
  metaEl.className = "meta";
  metaEl.textContent = "server";
  const body = document.createElement("div");
  body.className = "body";

  const header = document.createElement("div");
  header.className = "status-header";
  header.textContent = `Server Status`;
  if (payload.version) {
    const versionBadge = document.createElement("span");
    versionBadge.className = "status-version";
    versionBadge.textContent = `v${payload.version}`;
    header.appendChild(versionBadge);
  }
  body.appendChild(header);

  const table = document.createElement("table");
  table.className = "status-table";

  const addRow = (label, value, className = "") => {
    const row = document.createElement("tr");
    const labelCell = document.createElement("td");
    labelCell.className = "status-label";
    labelCell.textContent = label;
    const valueCell = document.createElement("td");
    valueCell.className = `status-value ${className}`;
    valueCell.textContent = value;
    row.appendChild(labelCell);
    row.appendChild(valueCell);
    table.appendChild(row);
  };

  // System info
  if (payload.os) {
    addRow("Platform", `${payload.os}${payload.cpuCores ? ` (${payload.cpuCores} cores)` : ""}`);
  }
  if (payload.rustVersion) {
    addRow("Rust", payload.rustVersion);
  }

  // Runtime stats
  addRow("Uptime", formatUptime(payload.uptimeSeconds));
  addRow("Users", `${payload.userCount}${payload.peakUsers ? ` (peak: ${payload.peakUsers})` : ""}`);
  if (payload.connectionsTotal !== undefined) {
    addRow("Connections", payload.connectionsTotal.toLocaleString());
  }
  addRow("Messages", payload.messagesSent.toLocaleString());
  if (payload.messagesPerSecond !== undefined) {
    addRow("Throughput", `${payload.messagesPerSecond} msg/s`);
  }
  if (payload.memoryMb !== undefined) {
    addRow("Memory", `${payload.memoryMb} MB`);
  }

  // AI status
  if (payload.aiEnabled !== undefined) {
    const aiStatus = payload.aiEnabled
      ? (payload.aiModel || "enabled")
      : "disabled";
    addRow("AI", aiStatus, payload.aiEnabled ? "status-ai-on" : "status-ai-off");
  }

  body.appendChild(table);

  // Subtle GitHub link
  const footer = document.createElement("div");
  footer.className = "status-footer";
  const link = document.createElement("a");
  link.href = "https://github.com/pa1bh/chatserver/";
  link.target = "_blank";
  link.rel = "noopener";
  link.textContent = "github";
  footer.appendChild(link);
  body.appendChild(footer);

  item.appendChild(metaEl);
  item.appendChild(body);
  messagesEl.appendChild(item);
  scrollToBottom();
};

const sendPayload = (payload) => {
  if (!socket || socket.readyState !== WebSocket.OPEN) {
    appendMessage("error", "Niet verbonden met server.", "client");
    return;
  }
  socket.send(JSON.stringify(payload));
};

const handleCommand = (raw) => {
  const [cmd, ...rest] = raw.slice(1).trim().split(/\s+/);
  if (!cmd) return appendMessage("error", "Onbekend commando.", "client");

  switch (cmd.toLowerCase()) {
    case "name":
      return sendPayload({ type: "setName", name: rest.join(" ") || nicknameInput.value || "" });
    case "status":
      return sendPayload({ type: "status" });
    case "users":
      return sendPayload({ type: "listUsers" });
    case "ping": {
      const token = rest[0] || generateToken();
      pendingPings.set(token, performance.now());
      return sendPayload({ type: "ping", token });
    }
    case "ai": {
      const prompt = rest.join(" ");
      if (!prompt) return appendMessage("error", "Gebruik: /ai <vraag>", "client");
      appendMessage("system", "AI denkt na...", "client");
      return sendPayload({ type: "ai", prompt });
    }
    default:
      return appendMessage("error", `Onbekend commando: /${cmd}`, "client");
  }
};

const handleMessage = (event) => {
  let payload;
  try {
    payload = JSON.parse(event.data);
  } catch {
    return appendMessage("error", "Kon bericht niet lezen.", "server");
  }

  switch (payload.type) {
    case "chat": {
      const isMine = payload.from === currentName;
      appendMessage("msg", payload.text, `${payload.from} • ${new Date(payload.at).toLocaleTimeString()}`, isMine);
      break;
    }
    case "system": {
      const isPresence = payload.text.endsWith("heeft de chat betreden.") ||
                         payload.text.endsWith("heeft de chat verlaten.") ||
                         / heet nu .+\.$/.test(payload.text);
      const msgType = isPresence ? "presence" : "system";
      appendMessage(msgType, payload.text, new Date(payload.at).toLocaleTimeString());
      break;
    }
    case "ackName":
      currentName = payload.name;
      nicknameInput.value = payload.name;
      if (currentNameEl) currentNameEl.textContent = payload.name;
      if (composerNameText) composerNameText.textContent = getInitials(payload.name);
      appendMessage("system", `Je heet nu ${payload.name}.`, new Date(payload.at).toLocaleTimeString());
      break;
    case "status": {
      appendStatus(payload);
      break;
    }
    case "listUsers": {
      if (!payload.users?.length) {
        appendMessage("system", "Geen gebruikers online.", "server");
      } else {
        appendUserList(payload.users);
      }
      break;
    }
    case "error":
      appendMessage("error", payload.message, "server");
      break;
    case "pong": {
      const token = payload.token;
      const sentAt = token ? pendingPings.get(token) : null;
      if (sentAt) {
        const roundtrip = (performance.now() - sentAt).toFixed(2);
        pendingPings.delete(token);
        appendMessage("system", `Pong! roundtrip: ${roundtrip}ms${token ? ` (token: ${token.slice(0, 8)}...)` : ""}`, "server");
      } else {
        appendMessage("system", `Pong!${token ? ` (token: ${token.slice(0, 8)}...)` : ""}`, "server");
      }
      break;
    }
    case "ai": {
      const isMine = payload.from === currentName;
      const time = new Date(payload.at).toLocaleTimeString();
      const formatted = `**Q:** ${payload.prompt}\n\n**A:** ${payload.response}`;
      // Build stats line
      const stats = [`${payload.responseMs}ms`];
      if (payload.tokens) stats.push(`${payload.tokens} tokens`);
      if (payload.cost !== undefined && payload.cost !== null) stats.push(`$${payload.cost.toFixed(4)}`);
      const meta = `${payload.from} • ${time} • ${stats.join(" | ")}`;
      appendMessage("ai", formatted, meta, isMine, true);
      break;
    }
    default:
      appendMessage("error", "Onbekend bericht van server.", "server");
  }
};

const connect = () => {
  clearTimeout(reconnectTimeout);
  setStatus("Verbinden...");
  socket = new WebSocket(WS_URL);

  socket.onopen = () => {
    setStatus("Verbonden");
    if (nicknameInput.value.trim()) {
      sendPayload({ type: "setName", name: nicknameInput.value.trim() });
    }
  };

  socket.onmessage = handleMessage;

  socket.onclose = () => {
    setStatus("Verbinding verbroken, opnieuw verbinden...", "warn");
    reconnectTimeout = setTimeout(connect, 1500);
  };

  socket.onerror = () => {
    setStatus("Verbindingsfout", "error");
  };
};

form?.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = input.value.trim();
  if (!text) return;
  if (text.startsWith("/")) {
    handleCommand(text);
  } else {
    sendPayload({ type: "chat", text });
  }
  input.value = "";
});

saveNameBtn?.addEventListener("click", () => {
  const name = nicknameInput.value.trim();
  if (!name) return appendMessage("error", "Voer een naam in om op te slaan.", "client");
  sendPayload({ type: "setName", name });
  nameModal?.classList.add("hidden");
});

const openNameModal = () => {
  nameModal?.classList.remove("hidden");
  nicknameInput?.focus();
  nicknameInput?.select();
};

const closeNameModal = () => {
  nameModal?.classList.add("hidden");
};

nameBadge?.addEventListener("click", openNameModal);
composerName?.addEventListener("click", openNameModal);
closeNameBtn?.addEventListener("click", closeNameModal);

nicknameInput?.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    saveNameBtn?.click();
  }
  if (event.key === "Escape") {
    closeNameModal();
  }
});

connect();

// iOS keyboard handling: scroll to bottom when keyboard appears
if (window.visualViewport) {
  window.visualViewport.addEventListener("resize", () => {
    scrollToBottom();
  });
}

// Also scroll when input is focused (keyboard appears)
input?.addEventListener("focus", () => {
  setTimeout(scrollToBottom, 300);
});
