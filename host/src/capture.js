// Capture source abstraction — Phase 1 (PROTOCOL.md §Display control).
// Capture binds to the VIRTUAL monitor only; monitor_id:-1 = mirror fallback.
// Backends "wgc"/"dxgi" exist in the Rust core; this Node host runs "test"
// (SSE moving-box pattern) so transport/RTT/overlay verify without a driver.

export const BACKENDS = ["test", "wgc", "dxgi"];

/** @type {{backend: string, monitor_id: number}} */
let source = { backend: "test", monitor_id: -1 };

export function currentSource() { return { ...source }; }

/**
 * @param {{backend?: string, monitor_id?: number}} sel
 * @returns {{ok: boolean, source?: any, reason?: string}}
 */
export function selectSource(sel = {}) {
  const backend = sel.backend || source.backend;
  if (!BACKENDS.includes(backend)) return { ok: false, reason: `unknown backend (want ${BACKENDS.join("|")})` };
  if (backend !== "test") return { ok: false, reason: `${backend} needs the Rust core (Phase 1 infile)` };
  source = { backend, monitor_id: typeof sel.monitor_id === "number" ? sel.monitor_id : source.monitor_id };
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
