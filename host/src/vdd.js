// VDD controller — Phase 1 (PROTOCOL.md §Display control).
// Talks to the signed VDD helper over \\.\pipe\extendo-vdd (JSON lines).
// No helper installed (typical dev box) → in-memory STUB registry, always
// flagged stub:true so callers never mistake it for a real IddCx monitor.
// Arrival/departure map to IddCxMonitorArrival / IddCxMonitorDeparture.
import { connect } from "node:net";

export const VDD_PIPE = "\\\\.\\pipe\\extendo-vdd";

/** EDID profiles shipped with the signed VDD fork (see driver/README.md). */
export const EDID_PROFILES = {
  "phone-720p": { width: 1280, height: 720, fps: 60 },
  "phone-1080p": { width: 1920, height: 1080, fps: 60 },
};

function sendPipe(payload, timeoutMs = 1500) {
  return new Promise((resolve) => {
    let buf = "";
    const s = connect(VDD_PIPE);
    const timer = setTimeout(() => { try { s.destroy(); } catch { /* noop */ } resolve(null); }, timeoutMs);
    s.on("connect", () => s.write(JSON.stringify(payload) + "\n"));
    s.on("data", (c) => {
      buf += c.toString();
      if (buf.includes("\n")) {
        clearTimeout(timer);
        try { s.end(); } catch { /* noop */ }
        try { resolve(JSON.parse(buf.trim())); } catch { resolve(null); }
      }
    });
    s.on("error", () => { clearTimeout(timer); resolve(null); });
  });
}

// In-memory stub (dev without driver). Resettable for tests.
let nextId = 1;
/** @type {Map<number, any>} */
const stubMonitors = new Map();

export function __resetStub() { stubMonitors.clear(); nextId = 1; }

function stubAdd(mode, edid) {
  const m = { id: nextId++, width: mode.width, height: mode.height, fps: mode.fps, active: true, edid: edid || "" };
  stubMonitors.set(m.id, m);
  return { ok: true, stub: true, monitor: m };
}

/** @param {{width:number,height:number,fps:number}} mode */
export async function addMonitor(mode, edid = "") {
  const live = await sendPipe({ cmd: "add", ...mode, edid });
  if (live && live.ok) return { ...live, stub: false };
  return stubAdd(mode, edid);
}

/** @param {number} id */
export async function removeMonitor(id) {
  const live = await sendPipe({ cmd: "remove", id });
  if (live && live.ok) {
    stubMonitors.delete(id);
    return { ...live, stub: false };
  }
  const existed = stubMonitors.delete(id);
  return { ok: existed, stub: true, ...(existed ? {} : { reason: "unknown-id" }) };
}

export async function listMonitors() {
  const live = await sendPipe({ cmd: "list" });
  if (live && live.ok) return { ok: true, stub: false, monitors: live.monitors };
  return { ok: true, stub: true, monitors: [...stubMonitors.values()] };
}

/** Reuse a matching active monitor or add one (idempotent extend). */
export async function ensureMonitor(mode, edid = "") {
  const list = await listMonitors();
  const hit = list.monitors.find(
    (m) => m.active && m.width === mode.width && m.height === mode.height && m.fps === mode.fps
  );
  if (hit) return { ok: true, stub: list.stub, monitor: hit, reused: true };
  const added = await addMonitor(mode, edid);
  return { ...added, reused: false };
}
