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

const getInitials = (name) => {
  if (!name) return "?";
  const parts = name.trim().split(/[-_\s]+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return name.slice(0, 2).toUpperCase();
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
      if (composerNameText) composerNameText.textContent = getInitials(payload.name);
      appendMessage("system", `Je heet nu ${payload.name}.`, new Date(payload.at).toLocaleTimeString());
      break;
    case "status": {
      const parts = [
        `users: ${payload.userCount}`,
        `uptime: ${payload.uptimeSeconds}s`,
        `msgs: ${payload.messagesSent}`,
      ];
      if (payload.messagesPerSecond !== undefined) {
        parts.push(`msg/s: ${payload.messagesPerSecond}`);
      }
      if (payload.memoryMb !== undefined) {
        parts.push(`mem: ${payload.memoryMb} MB`);
      }
      appendMessage("system", `Status: ${parts.join(" | ")}`, "server");
      break;
    }
    case "listUsers": {
      if (!payload.users?.length) {
        appendMessage("system", "Geen gebruikers online.", "server");
      } else {
        const lines = payload.users.map((u) => {
          const info = [u.name];
          if (u.ip) info.push(`ip: ${u.ip}`);
          info.push(`id: ${u.id.slice(0, 8)}...`);
          return info.join(" | ");
        });
        appendMessage("system", `Gebruikers (${payload.users.length}):\n${lines.join("\n")}`, "server");
      }
      break;
    }
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
