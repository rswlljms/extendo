// Host HTTP service — Phase 0 (PROTOCOL.md).
// Endpoints: /info /ping /stats /frames(SSE test pattern) /pair /input.
// Real H.264 capture (Rust) replaces only the frame payload in Phase 1.
import { createServer } from "node:http";
import { listCandidates, probeCandidates, pickBest } from "./transport.js";
import { canUse, validateLicense, maxVideoForTier } from "./license.js";
import { qrPayload } from "./config.js";

export const VERSION = "0.1.0";

/**
 * @param {any} cfg config object (see config.js)
 */
export function createHost(cfg) {
  const stats = { fps: cfg.video.fps, drops: 0, bitrate_kbps: cfg.video.bitrate_kbps, rtt_ms: -1 };
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

  const server = createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://x");
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET,POST,OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");
    if (req.method === "OPTIONS") { res.writeHead(204); res.end(); return; }

    if (url.pathname === "/info" && req.method === "GET") {
      const cands = listCandidates(cfg.port);
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({
        name: cfg.device_name, version: VERSION,
        transports: cands, video: cfg.video,
        qr: cands.filter((c) => c.kind !== "localhost").map((c) => qrPayload(c.addr, c.port, cfg.token)),
        tier: validateLicense(cfg.license).tier,
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
      const body = await readBody(req);
      const ok = body.token === cfg.token || body.token === cfg.pin;
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
      // Phase 2: route to SendInput(). Phase 0: validate shape + count.
      const valid = typeof body.type === "string";
      if (!valid) stats.drops += 1;
      res.writeHead(valid ? 200 : 400, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: valid }));
      return;
    }

    if (url.pathname === "/quality" && req.method === "POST") {
      // Gated by entitlement: free tier cannot exceed 720p30.
      const body = await readBody(req);
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

    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });

  return { server, stats };
}
