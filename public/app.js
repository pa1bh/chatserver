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

let socket;
let currentName = "";
let reconnectTimeout;

const setStatus = (text, tone = "info") => {
  connectionStatus.textContent = text;
  connectionStatus.style.background = tone === "error" ? "#fff1f2" : tone === "warn" ? "#fef9c3" : "#eef2ff";
  connectionStatus.style.color = tone === "error" ? "#9f1239" : tone === "warn" ? "#854d0e" : "#312e81";
};

const appendMessage = (type, text, meta = "", isMine = false) => {
  const item = document.createElement("div");
  item.className = `msg ${type} ${isMine ? "mine" : ""}`;
  const metaEl = document.createElement("div");
  metaEl.className = "meta";
  metaEl.textContent = meta;
  const body = document.createElement("div");
  body.textContent = text;
  item.appendChild(metaEl);
  item.appendChild(body);
  messagesEl.appendChild(item);
  requestAnimationFrame(() => {
    messagesEl.scrollTop = messagesEl.scrollHeight;
  });
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
    case "system":
      appendMessage("system", payload.text, new Date(payload.at).toLocaleTimeString());
      break;
    case "ackName":
      currentName = payload.name;
      nicknameInput.value = payload.name;
      if (currentNameEl) currentNameEl.textContent = payload.name;
      appendMessage("system", `Je heet nu ${payload.name}.`, new Date(payload.at).toLocaleTimeString());
      break;
    case "status":
      appendMessage("system", `Status: users=${payload.userCount}, uptime=${payload.uptimeSeconds}s, msgs=${payload.messagesSent}`, "server");
      break;
    case "listUsers":
      appendMessage("system", `Gebruikers: ${payload.users.map((u) => u.name).join(", ") || "niemand"}`, "server");
      break;
    case "error":
      appendMessage("error", payload.message, "server");
      break;
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
    sendPayload({ type: "status" });
    sendPayload({ type: "listUsers" });
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
