// extendo host entry — Phase 1. `node src/index.js [--port 9577]`
// Watches interfaces (USB tethering plug/unplug, Wi-Fi drop, resume) and
// re-probes the best transport so viewers can switch without re-pairing.
import { loadConfig, saveConfig } from "./config.js";
import { createHost } from "./server.js";
import { watchInterfaces } from "./iface.js";
import { probeCandidates, pickBest, listCandidates } from "./transport.js";
import { coreAvailable, ensureCore, stopCore } from "./capture.js";

const args = process.argv.slice(2);
const portIdx = args.indexOf("--port");
const cfg = loadConfig();
if (portIdx >= 0 && args[portIdx + 1]) cfg.port = Number(args[portIdx + 1]);

const { server, stats, adaptive, report } = createHost(cfg);
server.listen(cfg.port, "0.0.0.0", () => {
  saveConfig(cfg);
  console.log(`extendo host up on 0.0.0.0:${cfg.port} (PIN ${cfg.pin})`);
  if (coreAvailable()) {
    const r = ensureCore(cfg);
    if (r.ok) console.log(`capture core: wgc on 127.0.0.1:${cfg.port + 1} (pid ${r.pid}) — GET /video.mjpg`);
    else console.log(`capture core: ${r.reason}`);
  } else {
    console.log("capture core: not built (test-pattern fallback) — cargo build --release -p extendo-core");
  }
});

// Closed loop (Phase 2 adaptive): re-probe transports, feed RTT + viewer
// report into adaptive.check, apply downgrades/upgrades to cfg.video.
let lastReportDrops = 0;
setInterval(async () => {
  try {
    const probed = await probeCandidates(listCandidates(cfg.port));
    const best = pickBest(probed);
    if (best && best.rttMs >= 0) stats.rtt_ms = best.rttMs;
    const fresh = Date.now() - report.ts_ms < 15_000;
    const drops = fresh ? Math.max(0, report.drops - lastReportDrops) : 0;
    if (fresh) lastReportDrops = report.drops;
    const r = adaptive.check(
      { rttMs: stats.rtt_ms, fps: cfg.video.fps, drops, expectedFps: cfg.video.fps },
      cfg.video
    );
    if (r.changed) {
      stats.fps = cfg.video.fps;
      console.log(`adaptive: ${r.level} (${r.reason}) → ${cfg.video.width}x${cfg.video.height}@${cfg.video.fps}`);
    }
  } catch { /* never crash the host on a probe failure */ }
}, 5000).unref?.();

const watcher = watchInterfaces({
  onChange: async ({ addrs }) => {
    console.log(`iface change: ${addrs.join(", ") || "(none)"} — re-probing best transport`);
    const probed = await probeCandidates(listCandidates(cfg.port));
    const best = pickBest(probed);
    if (best) console.log(`best: ${best.kind} ${best.addr} rtt=${best.rttMs}ms`);
  },
});

process.on("SIGINT", () => { watcher.stop(); stopCore(); server.close(() => process.exit(0)); });
process.on("exit", () => { try { stopCore(); } catch { /* noop */ } });
