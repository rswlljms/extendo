// Transport abstraction (AGENTS.md §7.4): wifi | rndis | localhost.
// All video/input code uses Transport { kind, addr, port, rttMs }. No hardcoded IPs.
import { networkInterfaces } from "node:os";
import { connect } from "node:net";

/** @typedef {"wifi"|"rndis"|"localhost"} TransportKind */
/** @typedef {{kind: TransportKind, addr: string, port: number, rttMs: number}} Transport */

/** RNDIS / tethering ranges seen in the wild: 192.168.42.x, 192.168.43.x, 192.168.137.x */
export function isRndisIp(ip) {
  return (
    ip.startsWith("192.168.42.") ||
    ip.startsWith("192.168.43.") ||
    ip.startsWith("192.168.137.")
  );
}

export function isPrivateIp(ip) {
  if (ip.startsWith("10.")) return true;
  if (ip.startsWith("192.168.")) return true;
  const m = ip.match(/^172\.(\d+)\./);
  if (m) {
    const n = Number(m[1]);
    return n >= 16 && n <= 31;
  }
  return false;
}

/** All local IPv4 addrs (excluding internal). */
export function localIPv4Addrs() {
  const out = [];
  for (const ifaces of Object.values(networkInterfaces())) {
    for (const ni of ifaces || []) {
      if (ni && ni.family === "IPv4" && !ni.internal) out.push(ni.address);
    }
  }
  return out;
}

/**
 * Build candidate transports for a port. Unknown RTT = -1.
 * @param {number} port
 * @param {string[]} [addrs]
 * @returns {Transport[]}
 */
export function listCandidates(port, addrs = localIPv4Addrs()) {
  /** @type {Transport[]} */
  const out = [{ kind: "localhost", addr: "127.0.0.1", port, rttMs: -1 }];
  for (const addr of addrs) {
    if (!isPrivateIp(addr)) continue;
    out.push({ kind: isRndisIp(addr) ? "rndis" : "wifi", addr, port, rttMs: -1 });
  }
  return out;
}

const TIE_BREAK = { localhost: 0, rndis: 1, wifi: 2 };

/**
 * Pick lowest RTT; unknown (-1) sorts last; tie-break localhost > rndis > wifi.
 * @param {Transport[]} candidates
 * @returns {Transport|null}
 */
export function pickBest(candidates) {
  const known = candidates.filter((c) => c.rttMs >= 0);
  const pool = known.length ? known : candidates;
  if (!pool.length) return null;
  return [...pool].sort(
    (a, b) => (a.rttMs < 0 ? 1e9 : a.rttMs) - (b.rttMs < 0 ? 1e9 : b.rttMs) ||
      TIE_BREAK[a.kind] - TIE_BREAK[b.kind]
  )[0];
}

/**
 * Measure TCP-connect RTT to host:port. Resolves -1 on failure/timeout.
 * @param {string} host
 * @param {number} port
 * @param {number} [timeoutMs]
 * @returns {Promise<number>}
 */
export function probeRtt(host, port, timeoutMs = 800) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const s = connect(port, host);
    const done = (v) => {
      try { s.destroy(); } catch { /* noop */ }
      resolve(v);
    };
    const timer = setTimeout(() => done(-1), timeoutMs);
    s.on("connect", () => { clearTimeout(timer); done(Date.now() - t0); });
    s.on("error", () => { clearTimeout(timer); done(-1); });
  });
}

/**
 * Probe all candidates in parallel and return them with rttMs filled in.
 * @param {Transport[]} candidates
 * @returns {Promise<Transport[]>}
 */
export async function probeCandidates(candidates) {
  return Promise.all(
    candidates.map(async (c) => ({ ...c, rttMs: await probeRtt(c.addr, c.port) }))
  );
}
