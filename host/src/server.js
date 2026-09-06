// Host HTTP service — Phase 1 (PROTOCOL.md).
// Endpoints: /info /ping /stats /best /frames(SSE test pattern) /pair /input
//   /quality /displays /capture. Real H.264 capture (Rust) replaces only the
//   frame payload; display/capture contracts stay stable.
import { createServer } from "node:http";
import { listCandidates, probeCandidates, pickBest } from "./transport.js";
import { canUse, validateLicense, maxVideoForTier } from "./license.js";
import { addMonitor, removeMonitor, listMonitors, ensureMonitor, EDID_PROFILES } from "./vdd.js";
import { currentSource, selectSource, syncVideoToMode } from "./capture.js";
import { createInput } from "./input.js";
import { createAdaptive } from "./adaptive.js";
import { createThrottle } from "./throttle.js";
import { qrPayload } from "./config.js";

export const VERSION = "0.3.0";

/**
 * @param {any} cfg config object (see config.js)
 */
export function createHost(cfg) {
  const stats = { fps: cfg.video.fps, drops: 0, bitrate_kbps: cfg.video.bitrate_kbps, rtt_ms: -1 };
  const report = { fps: 0, drops: 0, ts_ms: 0 }; // last viewer POST /report
  const input = createInput();
  const adaptive = createAdaptive();
  const throttle = createThrottle();
  let seq = 0;
  const t0 = Date.now();

  const readBody = (req) =>
    new Promise((resolve) => {
      let s = "";
      req.on("data", (c) => { s += c; });
      req.on("end", () => {
        try { resolve(JSON.parse(s || "{}")); } catch { resolve({}); }
      });
    });

  // Mutation + video endpoints require the pairing token/PIN (header, query, or body).
  const authed = (req, url, body) =>
    req.headers["x-extendo-token"] === cfg.token ||
    req.headers["x-extendo-token"] === cfg.pin ||
    url.searchParams.get("token") === cfg.token ||
    url.searchParams.get("token") === cfg.pin ||
    (body && (body.token === cfg.token || body.token === cfg.pin));

  const needAuth = (res) => {
    res.writeHead(401, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "pairing token required" }));
    return true;
  };

  const server = createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://x");
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET,POST,OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");
    if (req.method === "OPTIONS") { res.writeHead(204); res.end(); return; }

    if (url.pathname === "/info" && req.method === "GET") {
      const cands = listCandidates(cfg.port);
      const displays = await listMonitors();
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({
        name: cfg.device_name, version: VERSION,
        transports: cands, video: cfg.video,
        qr: cands.filter((c) => c.kind !== "localhost").map((c) => qrPayload(c.addr, c.port, cfg.token)),
        tier: validateLicense(cfg.license).tier,
        displays: displays.monitors, displays_stub: displays.stub,
        capture: currentSource(),
      }));
      return;
    }

    if (url.pathname === "/ping" && req.method === "GET") {
      const clientT0 = Number(url.searchParams.get("t0") || 0);
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ t0: clientT0, server_ts_ms: Date.now() }));
      return;
    }

    if (url.pathname === "/stats" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ...stats, uptime_s: Math.floor((Date.now() - t0) / 1000) }));
      return;
    }

    if (url.pathname === "/best" && req.method === "GET") {
      const probed = await probeCandidates(listCandidates(cfg.port));
      const best = pickBest(probed);
      if (best && best.rttMs >= 0) stats.rtt_ms = best.rttMs;
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ best, probed }));
      return;
    }

    if (url.pathname === "/frames" && req.method === "GET") {
      if (!authed(req, url)) { needAuth(res); return; }
      // SSE test pattern: moving box, normalized coords. Viewer renders on canvas.
      res.writeHead(200, {
        "Content-Type": "text/event-stream", "Cache-Control": "no-cache", Connection: "keep-alive",
      });
      const period = Math.max(16, Math.floor(1000 / cfg.video.fps));
      const timer = setInterval(() => {
        seq += 1;
        const t = Date.now() / 1000;
        const frame = {
          seq, server_ts_ms: Date.now(),
          x: 0.5 + 0.4 * Math.sin(t), y: 0.5 + 0.35 * Math.cos(t * 0.7),
        };
        res.write(`data: ${JSON.stringify(frame)}\n\n`);
      }, period);
      req.on("close", () => clearInterval(timer));
      return;
    }

    if (url.pathname === "/pair" && req.method === "POST") {
      const ip = req.socket.remoteAddress || "unknown";
      if (throttle.blocked(ip)) {
        res.writeHead(429, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ accepted: false, reason: "too many attempts, retry later" }));
        return;
      }
      const body = await readBody(req);
      const ok = body.token === cfg.token || body.token === cfg.pin;
      throttle.attempt(ip, ok);
      const { tier } = validateLicense(cfg.license);
      const ceiling = maxVideoForTier(tier);
      const video = {
        ...cfg.video,
        width: Math.min(cfg.video.width, ceiling.width),
        height: Math.min(cfg.video.height, ceiling.height),
        fps: Math.min(cfg.video.fps, ceiling.fps),
      };
      res.writeHead(ok ? 200 : 403, { "Content-Type": "application/json" });
      res.end(JSON.stringify(ok
        ? { accepted: true, reason: "", transport: listCandidates(cfg.port)[0], video }
        : { accepted: false, reason: "bad token/PIN", transport: null, video: null }));
      return;
    }

    if (url.pathname === "/input" && req.method === "POST") {
      const body = await readBody(req);
      if (!authed(req, url, body)) { needAuth(res); return; }
      // Touch/keyboard backchannel → SendInput bridge (Phase 2). Touch/Pro-gated.
      const gated = body.type === "touch" || body.type === "key";
      if (gated) {
        const { tier } = validateLicense(cfg.license);
        if (!canUse("touch", tier)) {
          res.writeHead(402, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ ok: false, reason: "touch/keyboard input requires Pro", tier }));
          return;
        }
      }
      const r = input.handle(body);
      if (!r.ok) stats.drops += 1;
      res.writeHead(r.ok ? 200 : 400, { "Content-Type": "application/json" });
      res.end(JSON.stringify(r));
      return;
    }

    if (url.pathname === "/report" && req.method === "POST") {
      const body = await readBody(req);
      if (!authed(req, url, body)) { needAuth(res); return; }
      report.fps = Number(body.fps) || 0;
      report.drops = Number(body.drops) || 0;
      report.ts_ms = Date.now();
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, video: cfg.video, adaptive: adaptive.state.low ? "low" : "high" }));
      return;
    }

    if (url.pathname === "/quality" && req.method === "POST") {
      // Gated by entitlement: free tier cannot exceed 720p30.
      const body = await readBody(req);
      if (!authed(req, url, body)) { needAuth(res); return; }
      if (typeof body.auto === "boolean") adaptive.state.auto = body.auto;
      const { tier } = validateLicense(cfg.license);
      const ask1080p60 =
        (body.width || 0) > 1280 || (body.height || 0) > 720 || (body.fps || 0) > 30;
      if (ask1080p60 && !canUse("1080p60", tier)) {
        res.writeHead(402, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ ok: false, reason: "1080p60 requires Pro", tier }));
        return;
      }
      cfg.video = {
        width: body.width || cfg.video.width,
        height: body.height || cfg.video.height,
        fps: body.fps || cfg.video.fps,
        bitrate_kbps: body.bitrate_kbps || cfg.video.bitrate_kbps,
        codec: "h264",
      };
      stats.fps = cfg.video.fps;
      stats.bitrate_kbps = cfg.video.bitrate_kbps;
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, video: cfg.video }));
      return;
    }

    if (url.pathname === "/displays" && req.method === "GET") {
      if (!authed(req, url)) { needAuth(res); return; }
      const list = await listMonitors();
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify(list));
      return;
    }

    if (url.pathname === "/displays" && req.method === "POST") {
      // Body: DisplayAdd {mode:{width,height,fps}, edid}. "extend" creates-or-reuses.
      const body = await readBody(req);
      if (!authed(req, url, body)) { needAuth(res); return; }
      const extend = body.extend !== false;
      const profile = body.edid && EDID_PROFILES[body.edid];
      const mode = body.mode || profile || { width: 1280, height: 720, fps: 60 };
      const { tier } = validateLicense(cfg.license);
      const ceiling = maxVideoForTier(tier);
      if ((mode.width > ceiling.width || mode.height > ceiling.height || mode.fps > ceiling.fps) && !canUse("1080p60", tier)) {
        res.writeHead(402, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ ok: false, reason: "mode exceeds free tier (720p30); Pro unlocks 1080p60", tier }));
        return;
      }
      const multi = (await listMonitors()).monitors.filter((m) => m.active).length >= 1;
      if (multi && !canUse("multiMonitor", tier) && !extend) {
        res.writeHead(402, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ ok: false, reason: "multi-monitor requires Pro", tier }));
        return;
      }
      const r = extend ? await ensureMonitor(mode, body.edid || "") : await addMonitor(mode, body.edid || "");
      if (r.ok) {
        syncVideoToMode(cfg.video, { width: r.monitor.width, height: r.monitor.height, fps: Math.min(r.monitor.fps, ceiling.fps) });
        stats.fps = cfg.video.fps;
      }
      res.writeHead(r.ok ? 200 : 500, { "Content-Type": "application/json" });
      res.end(JSON.stringify(r));
      return;
    }

    if (url.pathname.startsWith("/displays/") && req.method === "DELETE") {
      if (!authed(req, url)) { needAuth(res); return; }
      const id = Number(url.pathname.split("/")[2]);
      const r = await removeMonitor(id);
      res.writeHead(r.ok ? 200 : 404, { "Content-Type": "application/json" });
      res.end(JSON.stringify(r));
      return;
    }

    if (url.pathname === "/capture" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, source: currentSource() }));
      return;
    }

    if (url.pathname === "/capture" && req.method === "POST") {
      const body = await readBody(req);
      if (!authed(req, url, body)) { needAuth(res); return; }
      const r = selectSource(body);
      res.writeHead(r.ok ? 200 : 400, { "Content-Type": "application/json" });
      res.end(JSON.stringify(r));
      return;
    }

    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });

  return { server, stats, input, adaptive, report };
}
