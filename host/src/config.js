// Config — JSON file only. No DB in v1 (AGENTS.md §7.6).
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { homedir, hostname } from "node:os";
import { join, dirname } from "node:path";
import { randomBytes } from "node:crypto";

export const DEFAULT_PORT = 9577;

export function defaultConfig() {
  return {
    version: 1,
    device_name: hostname() || "extendo-host",
    port: DEFAULT_PORT,
    pin: String(1000 + Math.floor(Math.random() * 9000)),
    token: randomBytes(16).toString("hex"),
    video: { width: 1280, height: 720, fps: 30, bitrate_kbps: 4000, codec: "h264" },
    license: { key: "", tier: "free", device_id: "" },
  };
}

export function configPath() {
  const base = process.env.APPDATA || join(homedir(), ".config");
  return join(base, "extendo", "config.json");
}

/** @param {string} [p] */
export function loadConfig(p = configPath()) {
  try {
    return { ...defaultConfig(), ...JSON.parse(readFileSync(p, "utf8")) };
  } catch {
    return defaultConfig();
  }
}

/** @param {any} cfg @param {string} [p] */
export function saveConfig(cfg, p = configPath()) {
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, JSON.stringify(cfg, null, 2));
  return p;
}

/** QR payload from PROTOCOL.md: extendo://ip:port?token=… */
export function qrPayload(ip, port, token) {
  return `extendo://${ip}:${port}?token=${encodeURIComponent(token)}`;
}
