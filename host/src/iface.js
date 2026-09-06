// Interface watcher — Phase 1 USB/replug reconnect.
// Polls local IPv4 set; fires onChange when it changes (USB tethering
// plugged/unplugged, Wi-Fi drop, ADB iface up). Host re-probes /best there.
import { localIPv4Addrs } from "./transport.js";

export function signature(addrs = localIPv4Addrs()) {
  return [...addrs].sort().join(",");
}

/**
 * @param {{intervalMs?: number, onChange?: (info:any)=>void, list?: ()=>string[]}} opts
 * @returns {{stop: ()=>void, check: ()=>boolean}}
 */
export function watchInterfaces({ intervalMs = 2000, onChange = () => {}, list = localIPv4Addrs } = {}) {
  let last = signature(list());
  const check = () => {
    const now = signature(list());
    if (now !== last) {
      last = now;
      onChange({ addrs: list(), signature: now });
      return true;
    }
    return false;
  };
  const timer = setInterval(check, intervalMs);
  timer.unref?.();
  return { stop: () => clearInterval(timer), check };
}
