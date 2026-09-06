// Adaptive quality — Phase 2 (PROTOCOL.md §Adaptive).
// Policy: drop to 720p30 when RTT>80ms or drop-rate>5%; restore when
// RTT<40ms and drops==0 for RESTORE_GOODS consecutive checks. Cooldown
// prevents flapping. Pure logic — host tick feeds it, tests drive it.
export const DROP_RTT_MS = 80;
export const DROP_RATE = 0.05;
export const RESTORE_RTT_MS = 40;
export const RESTORE_GOODS = 4;
export const COOLDOWN_MS = 10_000;

export const LOW = { width: 1280, height: 720, fps: 30 };

export function createAdaptive() {
  const state = {
    auto: true,           // viewer/host may disable via POST /quality {"auto":false}
    low: false,           // currently downgraded
    goods: 0,             // consecutive good checks
    lastChange: 0,
    /** @type {any} video config to restore (captured on first drop) */
    prev: null,
  };

  /**
   * @param {{rttMs:number, fps:number, drops:number, expectedFps:number}} s
   * @param {any} video live cfg.video (mutated on change)
   * @param {number} [now]
   * @returns {{changed:boolean, level:string, reason:string}}
   */
  const check = (s, video, now = Date.now()) => {
    if (!state.auto) return { changed: false, level: state.low ? "low" : "high", reason: "manual" };
    const rate = s.expectedFps > 0 ? s.drops / s.expectedFps : 0;
    const bad = s.rttMs > DROP_RTT_MS || rate > DROP_RATE;
    const good = s.rttMs >= 0 && s.rttMs < RESTORE_RTT_MS && s.drops === 0;
    const cooled = now - state.lastChange >= COOLDOWN_MS;

    if (!state.low && bad && cooled) {
      state.prev = { ...video };
      video.width = LOW.width; video.height = LOW.height; video.fps = LOW.fps;
      state.low = true; state.goods = 0; state.lastChange = now;
      return { changed: true, level: "low", reason: s.rttMs > DROP_RTT_MS ? `rtt ${s.rttMs}ms>80ms` : `drops ${(rate * 100).toFixed(1)}%>5%` };
    }
    if (state.low) {
      state.goods = good ? state.goods + 1 : 0;
      if (state.goods >= RESTORE_GOODS && cooled && state.prev) {
        Object.assign(video, state.prev);
        state.low = false; state.goods = 0; state.lastChange = now; state.prev = null;
        return { changed: true, level: "high", reason: "network recovered" };
      }
    }
    return { changed: false, level: state.low ? "low" : "high", reason: bad ? "bad-cooldown" : "steady" };
  };

  return { state, check };
}
