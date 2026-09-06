// Input backchannel — Phase 2 (PROTOCOL.md §Input).
// InputEvent {type,x,y,key_code,down,dx,dy} → runner actions.
// Windows: persistent tools/input-runner.ps1 via STDIN (no per-event spawn).
// Elsewhere / EXTENDO_INPUT_DRY=1: dry-run counter (tests, dev on other OS).
import { spawn } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ABS_MAX = 65535;

/** Normalized 0..1 → absolute 0..65535 (screen-agnostic, primary display). */
export function toAbsolute(v) {
  return Math.max(0, Math.min(ABS_MAX, Math.round(Number(v) * ABS_MAX)));
}

/**
 * Map one InputEvent to runner actions.
 * @param {{type:string,x?:number,y?:number,key_code?:number,down?:boolean,dx?:number,dy?:number}} ev
 * @returns {Array<Record<string, any>>}
 */
export function mapEvent(ev) {
  switch (ev.type) {
    case "touch":
      if (ev.down === false) return [{ a: "left", down: false }];
      if (ev.x === undefined || ev.y === undefined) return [];
      if (ev.down === true) return [{ a: "move", x: toAbsolute(ev.x), y: toAbsolute(ev.y) }, { a: "left", down: true }];
      return [{ a: "move", x: toAbsolute(ev.x), y: toAbsolute(ev.y) }];
    case "mouse_move":
      if (ev.dx !== undefined || ev.dy !== undefined) return [{ a: "rel", dx: ev.dx || 0, dy: ev.dy || 0 }];
      if (ev.x !== undefined && ev.y !== undefined) return [{ a: "move", x: toAbsolute(ev.x), y: toAbsolute(ev.y) }];
      return [];
    case "mouse_down": return [{ a: "left", down: true }];
    case "mouse_up": return [{ a: "left", down: false }];
    case "scroll": return [{ a: "wheel", d: (ev.dy || 0) * -12 }]; // dy>0 (down) → negative wheel
    case "key":
      if (!ev.key_code) return [];
      return [{ a: "key", vk: ev.key_code & 0xff, down: ev.down !== false }];
    default: return [];
  }
}

export function isDryRun() {
  return process.platform !== "win32" || process.env.EXTENDO_INPUT_DRY === "1";
}

/** Persistent runner. dry:true never spawns (counts only). */
export function createInput() {
  const state = { count: 0, dry: isDryRun(), alive: false };
  /** @type {import("node:child_process").ChildProcessWithoutNullStreams | null} */
  let child = null;

  const ensure = () => {
    if (state.dry || child) return;
    const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
    child = spawn("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", join(root, "tools", "input-runner.ps1")], { stdio: ["pipe", "ignore", "ignore"] });
    state.alive = true;
    child.on("exit", () => { child = null; state.alive = false; });
    child.on("error", () => { child = null; state.alive = false; });
  };

  /**
   * @param {any} ev InputEvent
   * @returns {{ok: boolean, actions: number, dry: boolean, reason?: string}}
   */
  const handle = (ev) => {
    if (!ev || typeof ev.type !== "string") return { ok: false, actions: 0, dry: state.dry, reason: "bad-shape" };
    const actions = mapEvent(ev);
    if (!actions.length) return { ok: false, actions: 0, dry: state.dry, reason: "unmapped" };
    if (!state.dry) {
      ensure();
      if (!child || !child.stdin.writable) return { ok: false, actions: 0, dry: false, reason: "runner-dead" };
      for (const a of actions) child.stdin.write(JSON.stringify(a) + "\n");
    }
    state.count += actions.length;
    return { ok: true, actions: actions.length, dry: state.dry };
  };

  const stop = () => { try { child?.stdin.end(); child?.kill(); } catch { /* noop */ } child = null; };
  return { handle, stop, state };
}
