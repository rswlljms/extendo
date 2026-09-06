// extendo web viewer — Phase 0. Renders host /frames SSE test pattern on
// canvas with RTT/fps overlay. Same field names as extendo.proto (JSON form).
// Usage: open index.html, enter host (e.g. http://192.168.1.10:9577), Connect.

interface VideoConfig { width: number; height: number; fps: number; bitrate_kbps: number; codec: string }
interface Frame { seq: number; server_ts_ms: number; x: number; y: number }
interface Transport { kind: string; addr: string; port: number; rttMs: number }

const $ = (id: string): HTMLInputElement => document.getElementById(id) as HTMLInputElement;
const hostEl = $("host"), tokenEl = $("token"), overlayEl = $("overlay");
const canvas = document.getElementById("screen") as HTMLCanvasElement;
const ctx = canvas.getContext("2d") as CanvasRenderingContext2D;

let host = "";
let authToken = "";
let frameCount = 0, lastFpsT = performance.now(), fps = 0, rttMs = -1, drops = 0, lastSeq = 0;
let rotated = false;
let adaptiveLevel = "";

function api(path: string, init?: RequestInit): Promise<Response> {
  return fetch(`${host}${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", "x-extendo-token": authToken, ...(init?.headers ?? {}) },
  });
}

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
  overlayEl.value = `rtt ${rttMs}ms · fps ${fps} · drops ${drops} · ${W}x${H}${adaptiveLevel ? " · auto:" + adaptiveLevel : ""}`;
}

async function connect(): Promise<void> {
  host = hostEl.value.replace(/\/$/, "");
  authToken = tokenEl.value;
  const pair = await fetch(`${host}/pair`, {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ device_name: "web-viewer", token: authToken }),
  });
  if (!pair.ok) { overlayEl.value = pair.status === 429 ? "too many attempts, retry later" : "pair failed: bad token/PIN"; return; }
  const { video } = await pair.json() as { video: VideoConfig };
  void pingLoop();
  void reportLoop();
  void renderTransports();
  streamFrames(video);
}

let es: EventSource | null = null;
let reconnects = 0;

function streamFrames(video: VideoConfig): void {
  es?.close();
  es = new EventSource(`${host}/frames?token=${encodeURIComponent(authToken)}`);
  es.onopen = () => { reconnects = 0; };
  es.onmessage = (e: MessageEvent) => draw(JSON.parse(e.data) as Frame, video);
  // USB replug / sleep / Wi-Fi drop: back off and resume without re-pairing.
  (es as unknown as { onerror: unknown }).onerror = () => {
    drops += 1;
    es?.close();
    reconnects += 1;
    overlayEl.value = `reconnecting… (attempt ${reconnects})`;
    setTimeout(() => { if (host) streamFrames(video); }, Math.min(5000, 500 * reconnects));
  };
}

// Phase 1: list probed transports with RTT; tap one to switch (Wi-Fi ↔ USB)
// without re-pairing — token stays the same, only the base URL changes.
async function renderTransports(): Promise<void> {
  try {
    const r = await fetch(`${host}/best`);
    if (!r.ok) return;
    const { best, probed } = await r.json() as { best: Transport; probed: Transport[] };
    const bar = document.getElementById("transports");
    if (!bar) return;
    bar.innerHTML = "";
    for (const t of probed) {
      const b = document.createElement("button");
      b.textContent = `${t.kind} ${t.addr} ${t.rttMs >= 0 ? t.rttMs + "ms" : "?"}`;
      if (best && t.addr === best.addr) b.style.border = "2px solid #38bdf8";
      b.addEventListener("click", () => {
        host = `http://${t.addr}:${t.port}`;
        hostEl.value = host;
        overlayEl.value = `switched to ${t.kind} ${t.addr}`;
      });
      bar.appendChild(b);
    }
  } catch { /* host without /best (Phase 0) — skip */ }
}

async function setQuality(): Promise<void> {
  const [w, h] = ($("quality").value as string).split("x").map(Number);
  const fpsAsk = Number(($("fps").value as string));
  const r = await api(`/quality`, {
    method: "POST", body: JSON.stringify({ width: w, height: h, fps: fpsAsk }),
  });
  overlayEl.value = r.status === 402 ? "1080p60 requires Pro (free capped at 720p30)" : `quality ${w}x${h}@${fpsAsk}`;
}

// Phase 2 input: canvas touch → host SendInput bridge (Pro-gated on free tier).
async function sendInput(ev: Record<string, unknown>): Promise<void> {
  if (!host || !authToken) return;
  try {
    const r = await api(`/input`, { method: "POST", body: JSON.stringify(ev) });
    if (r.status === 402) overlayEl.value = "touch/keyboard input requires Pro";
  } catch { drops += 1; }
}

function canvasPos(e: PointerEvent): { x: number; y: number } {
  const r = canvas.getBoundingClientRect();
  return { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height };
}

canvas.addEventListener("pointerdown", (e) => {
  canvas.setPointerCapture(e.pointerId);
  const p = canvasPos(e);
  void sendInput({ type: "touch", x: p.x, y: p.y, down: true });
});
canvas.addEventListener("pointermove", (e) => {
  if (!(e.buttons & 1)) return;
  const p = canvasPos(e);
  void sendInput({ type: "touch", x: p.x, y: p.y });
});
canvas.addEventListener("pointerup", () => { void sendInput({ type: "touch", down: false }); });
document.addEventListener("keydown", (e) => {
  if ((e.target as HTMLElement)?.tagName === "INPUT") return;
  void sendInput({ type: "key", key_code: e.keyCode, down: true });
});
document.addEventListener("keyup", (e) => {
  if ((e.target as HTMLElement)?.tagName === "INPUT") return;
  void sendInput({ type: "key", key_code: e.keyCode, down: false });
});

// Closed loop: report observed fps/drops so the host adaptive policy can act.
async function reportLoop(): Promise<void> {
  while (host) {
    try {
      const r = await api(`/report`, { method: "POST", body: JSON.stringify({ fps, drops }) });
      if (r.ok) adaptiveLevel = ((await r.json()) as { adaptive: string }).adaptive;
    } catch { /* next tick */ }
    await new Promise((r) => setTimeout(r, 5000));
  }
}

document.getElementById("connect")?.addEventListener("click", () => void connect());
document.getElementById("apply")?.addEventListener("click", () => void setQuality());
document.getElementById("rotate")?.addEventListener("click", () => { rotated = !rotated; });
