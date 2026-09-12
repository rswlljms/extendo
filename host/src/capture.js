// Capture source abstraction — Phase 1 (PROTOCOL.md §Display control).
// Capture binds to the VIRTUAL monitor only; monitor_id:-1 = mirror fallback.
// "test" is the SSE moving-box fallback so transport/RTT verify without a
// driver. "wgc" is the Rust core (host/core -> WGC -> JPEG -> MJPEG) added in
// 0.5.0; when the binary is present it replaces the test pattern with real
// pixels on the same HTTP contract.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { get } from "node:http";

export const BACKENDS = ["test", "wgc", "dxgi"];

/** @type {{backend: string, monitor_id: number}} */
let source = { backend: "test", monitor_id: -1 };

/** @type {import("node:child_process").ChildProcess | null} */
let coreProc = null;
let corePort = 0;
let coreArgs = "";

export function currentSource() {
  return { ...source, core: coreState() };
}

function coreState() {
  if (!coreProc || coreProc.exitCode !== null) return { running: false, port: corePort, pid: null };
  return { running: true, port: corePort, pid: coreProc.pid || null };
}

function coreBinaryCandidates() {
  let here;
  try {
    here = dirname(fileURLToPath(import.meta.url));
  } catch {
    // Fallback for test harnesses where import.meta.url may be unavailable.
    here = join(process.cwd(), "host", "src");
  }
  return [
    join(here, "..", "core", "target", "release", "extendo-core.exe"),
    join(here, "..", "core", "target", "debug", "extendo-core.exe"),
    join(here, "..", "bin", "extendo-core.exe"),
  ];
}

export function coreBinaryPath() {
  for (const p of coreBinaryCandidates()) if (existsSync(p)) return p;
  return null;
}

export function coreAvailable() { return coreBinaryPath() !== null; }

/**
 * Start (or restart) the Rust core with caps derived from cfg.video.
 * No-ops with a reason when the binary has not been built.
 * The Node host owns entitlement (AGENTS.md 10.1) - caps are already
 * ceiling-clamped before they reach here, so this file never gates tiers.
 * @param {any} cfg config object (cfg.video, cfg.token, cfg.port)
 * @returns {{ok: boolean, reason?: string, pid?: number}}
 */
export function ensureCore(cfg) {
  const bin = coreBinaryPath();
  if (!bin) return { ok: false, reason: "Rust core not built (run: cargo build --release -p extendo-core)" };
  const wantPort = (cfg.port || 9577) + 1;
  const wantArgs = [
    "--port", String(wantPort),
    "--bind", "127.0.0.1",
    "--token", cfg.token || "",
    "--monitor", String(source.monitor_id >= 0 ? source.monitor_id + 1 : 0),
    "--width", String(cfg.video.width),
    "--height", String(cfg.video.height),
    "--fps", String(cfg.video.fps),
    "--quality", "70",
    "--codec", cfg.video.codec === "h264" ? "h264" : "mjpeg",
    "--bitrate", String(cfg.video.bitrate_kbps || 4000),
  ].join(" ");
  if (coreProc && coreProc.exitCode === null && corePort === wantPort && coreArgs === wantArgs) {
    return { ok: true, pid: coreProc.pid || undefined };
  }
  stopCore();
  corePort = wantPort;
  coreArgs = wantArgs;
  const args = wantArgs.split(" ");
  coreProc = spawn(bin, args, { stdio: ["ignore", "inherit", "inherit"], detached: false });
  coreProc.on("exit", (code) => {
    console.log(`capture core exited (${code})`);
    coreProc = null;
  });
  return { ok: true, pid: coreProc.pid || undefined };
}

export function stopCore() {
  if (coreProc && coreProc.exitCode === null) {
    try { coreProc.kill(); } catch { /* noop */ }
  }
  coreProc = null;
}

export function coreHealthUrl() {
  if (!corePort) return null;
  return `http://127.0.0.1:${corePort}/health`;
}

/** Fetch JSON from the core's /health (loopback, no auth). */
export function fetchCoreHealth(timeoutMs = 1500) {
  return new Promise((resolve) => {
    const url = coreHealthUrl();
    if (!url || !corePort) { resolve(null); return; }
    const req = get(url, (res) => {
      let body = "";
      res.on("data", (c) => { body += c; });
      res.on("end", () => {
        try { resolve(JSON.parse(body)); } catch { resolve(null); }
      });
    });
    req.on("error", () => resolve(null));
    req.setTimeout(timeoutMs, () => { req.destroy(); resolve(null); });
  });
}

/**
 * @param {{backend?: string, monitor_id?: number}} sel
 * @param {any} [cfg] when backend is wgc and cfg is provided, spawn the core
 * @returns {{ok: boolean, source?: any, reason?: string}}
 */
export function selectSource(sel = {}, cfg = null) {
  const backend = sel.backend || source.backend;
  if (!BACKENDS.includes(backend)) return { ok: false, reason: `unknown backend (want ${BACKENDS.join("|")})` };
  if (backend === "wgc") {
    if (!coreAvailable()) return { ok: false, reason: "wgc needs the Rust core (cargo build --release -p extendo-core)" };
    if (cfg) {
      const r = ensureCore(cfg);
      if (!r.ok) return r;
    }
    source = { backend, monitor_id: typeof sel.monitor_id === "number" ? sel.monitor_id : source.monitor_id };
    return { ok: true, source: currentSource() };
  }
  if (backend === "dxgi") return { ok: false, reason: "dxgi needs the Rust core (Phase 1 infile)" };
  source = { backend, monitor_id: typeof sel.monitor_id === "number" ? sel.monitor_id : source.monitor_id };
  if (backend === "test") stopCore();
  return { ok: true, source: currentSource() };
}

/**
 * Sync VideoConfig to a monitor mode (caller caps by tier ceiling first).
 * @param {any} video cfg.video
 * @param {{width:number,height:number,fps:number}} mode
 */
export function syncVideoToMode(video, mode) {
  video.width = mode.width;
  video.height = mode.height;
  video.fps = mode.fps;
  return video;
}
