import { appendFile } from "fs/promises";

type LogLevel = "info" | "warn" | "error";

type LogTarget =
  | { kind: "stdout" }
  | { kind: "file"; path: string; writer: (text: string) => Promise<void> };

const LOG_ENV_TARGET = process.env.LOG_TARGET;
const LOG_ENV_FILE = process.env.LOG_FILE;

function parseCliLogTarget(): LogTarget | null {
  const arg = process.argv.find((item) => item.startsWith("--log="));
  if (!arg) return null;
  const value = arg.slice("--log=".length);
  if (value === "stdout") return { kind: "stdout" };
  if (value.startsWith("file:")) {
    const path = value.slice("file:".length) || LOG_ENV_FILE || "server.log";
    return {
      kind: "file",
      path,
      writer: async (text: string) => {
        await appendFile(path, text);
      },
    };
  }
  return null;
}

function parseEnvLogTarget(): LogTarget | null {
  if (LOG_ENV_TARGET === "stdout" || !LOG_ENV_TARGET) return { kind: "stdout" };
  if (LOG_ENV_TARGET === "file") {
    const path = LOG_ENV_FILE || "server.log";
    return {
      kind: "file",
      path,
      writer: async (text: string) => {
        await appendFile(path, text);
      },
    };
  }
  return null;
}

function resolveLogTarget(): LogTarget {
  const fromCli = parseCliLogTarget();
  if (fromCli) return fromCli;
  const fromEnv = parseEnvLogTarget();
  if (fromEnv) return fromEnv;
  return { kind: "stdout" };
}

export function createLogger(scope: string) {
  const target = resolveLogTarget();

  const emit = async (level: LogLevel, message: string, extra?: Record<string, unknown>) => {
    const timestamp = new Date().toISOString();
    const base = `[${timestamp}] [${scope}] [${level.toUpperCase()}] ${message}`;
    const body = extra ? `${base} ${JSON.stringify(extra)}` : base;

    if (target.kind === "stdout") {
      const out = level === "error" ? console.error : console.log;
      out(body);
    } else {
      await target.writer(body + "\n");
    }
  };

  return {
    info: (message: string, extra?: Record<string, unknown>) => void emit("info", message, extra),
    warn: (message: string, extra?: Record<string, unknown>) => void emit("warn", message, extra),
    error: (message: string, extra?: Record<string, unknown>) => void emit("error", message, extra),
    target: target.kind === "stdout" ? "stdout" : `file:${target.path}`,
  };
}
