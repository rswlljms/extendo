// Pair-attempt throttle — Phase 2. Locks an IP out after maxFails
// bad tokens inside windowMs. Pure + injectable clock for tests.

/**
 * @param {{maxFails?: number, windowMs?: number, lockMs?: number, now?: ()=>number}} [opts]
 */
export function createThrottle({ maxFails = 5, windowMs = 60_000, lockMs = 30_000, now = Date.now } = {}) {
  /** @type {Map<string, {fails: number[], until: number}>} */
  const ips = new Map();
  const blocked = (ip) => {
    const e = ips.get(ip);
    return !!e && e.until > now();
  };
  /** Record an attempt; ok=true resets. Returns {blocked:boolean}. */
  const attempt = (ip, ok) => {
    const t = now();
    let e = ips.get(ip);
    if (!e) { e = { fails: [], until: 0 }; ips.set(ip, e); }
    if (e.until > t) return { blocked: true };
    if (ok) { e.fails = []; e.until = 0; return { blocked: false }; }
    e.fails = [...e.fails.filter((f) => t - f < windowMs), t];
    if (e.fails.length >= maxFails) e.until = t + lockMs;
    return { blocked: false };
  };
  return { blocked, attempt };
}
