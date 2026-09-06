// extendo web viewer — Phase 0. Renders host /frames SSE test pattern on
// canvas with RTT/fps overlay. Same field names as extendo.proto (JSON form).
// Usage: open index.html, enter host (e.g. http://192.168.1.10:9577), Connect.

interface VideoConfig { width: number; height: number; fps: number; bitrate_kbps: number; codec: string }
interface Frame { seq: number; server_ts_ms: number; x: number; y: number }

const $ = (id: string): HTMLInputElement => document.getElementById(id) as HTMLInputElement;
const hostEl = $("host"), tokenEl = $("token"), overlayEl = $("overlay");
const canvas = document.getElementById("screen") as HTMLCanvasElement;
const ctx = canvas.getContext("2d") as CanvasRenderingContext2D;

let host = "";
let frameCount = 0, lastFpsT = performance.now(), fps = 0, rttMs = -1, drops = 0, lastSeq = 0;
let rotated = false;

async function pingLoop(): Promise<void> {
  while (host) {
    try {
      const t0 = Date.now();
      const r = await fetch(`${host}/ping?t0=${t0}`);
      if (r.ok) { await r.json(); rttMs = Date.now() - t0; }
    } catch { drops += 1; }
    await new Promise((r) => setTimeout(r, 1000));
  }
}

function draw(f: Frame, video: VideoConfig): void {
  const W = rotated ? video.height : video.width;
  const H = rotated ? video.width : video.height;
  if (canvas.width !== 320 || canvas.height !== Math.round(320 * H / W)) {
    canvas.height = Math.round(320 * H / W);
  }
  frameCount += 1;
  if (f.seq !== lastSeq + 1 && lastSeq !== 0) drops += f.seq - lastSeq - 1;
  lastSeq = f.seq;
  const now = performance.now();
  if (now - lastFpsT >= 1000) { fps = frameCount; frameCount = 0; lastFpsT = now; }

  ctx.fillStyle = "#0b0e14"; ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "#1f2937";
  for (let x = 0; x < canvas.width; x += 32) { ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, canvas.height); ctx.stroke(); }
  const bx = f.x * canvas.width, by = f.y * canvas.height;
  ctx.fillStyle = "#38bdf8"; ctx.fillRect(bx - 24, by - 24, 48, 48);
  ctx.fillStyle = "#e2e8f0"; ctx.font = "12px system-ui";
  ctx.fillText(`seq ${f.seq}`, 8, 16);
  overlayEl.value = `rtt ${rttMs}ms · fps ${fps} · drops ${drops} · ${W}x${H}`;
}

async function connect(): Promise<void> {
  host = hostEl.value.replace(/\/$/, "");
  const pair = await fetch(`${host}/pair`, {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ device_name: "web-viewer", token: tokenEl.value }),
  });
  if (!pair.ok) { overlayEl.value = "pair failed: bad token/PIN"; return; }
  const { video } = await pair.json() as { video: VideoConfig };
  void pingLoop();
  const es = new EventSource(`${host}/frames`);
  es.onmessage = (e: MessageEvent) => draw(JSON.parse(e.data) as Frame, video);
  (es as unknown as { onerror: unknown }).onerror = () => { drops += 1; };
}

async function setQuality(): Promise<void> {
  const [w, h] = ($("quality").value as string).split("x").map(Number);
  const fpsAsk = Number(($("fps").value as string));
  const r = await fetch(`${host}/quality`, {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ width: w, height: h, fps: fpsAsk }),
  });
  overlayEl.value = r.status === 402 ? "1080p60 requires Pro (free capped at 720p30)" : `quality ${w}x${h}@${fpsAsk}`;
}

document.getElementById("connect")?.addEventListener("click", () => void connect());
document.getElementById("apply")?.addEventListener("click", () => void setQuality());
document.getElementById("rotate")?.addEventListener("click", () => { rotated = !rotated; });
