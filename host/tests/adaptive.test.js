import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createAdaptive, LOW } from "../src/adaptive.js";
import { createThrottle } from "../src/throttle.js";

const vid = (w = 1920, h = 1080, fps = 60) => ({ width: w, height: h, fps });

describe("adaptive quality", () => {
  it("drops to 720p30 on high RTT, remembers previous", () => {
    const a = createAdaptive();
    const v = vid();
    const r = a.check({ rttMs: 120, fps: 60, drops: 0, expectedFps: 60 }, v, 20_000);
    assert.equal(r.changed, true);
    assert.equal(r.level, "low");
    assert.deepEqual(v, { ...LOW });
  });

  it("drops on drop-rate >5%", () => {
    const a = createAdaptive();
    const v = vid();
    const r = a.check({ rttMs: 10, fps: 60, drops: 4, expectedFps: 60 }, v, 20_000);
    assert.equal(r.changed, true);
    assert.match(r.reason, /drops/);
  });

  it("holds during cooldown, restores after sustained good", () => {
    const a = createAdaptive();
    const v = vid();
    a.check({ rttMs: 200, fps: 60, drops: 0, expectedFps: 60 }, v, 20_000); // drop
    assert.equal(a.check({ rttMs: 200, fps: 30, drops: 0, expectedFps: 30 }, v, 21_000).changed, false); // cooldown
    let r;
    for (let i = 0; i < 4; i++) r = a.check({ rttMs: 10, fps: 30, drops: 0, expectedFps: 30 }, v, 40_000 + i);
    assert.equal(r.changed, true);
    assert.equal(r.level, "high");
    assert.deepEqual(v, { width: 1920, height: 1080, fps: 60 });
  });

  it("manual mode never changes", () => {
    const a = createAdaptive();
    a.state.auto = false;
    const v = vid();
    assert.equal(a.check({ rttMs: 500, fps: 60, drops: 60, expectedFps: 60 }, v).changed, false);
  });
});

describe("pair throttle", () => {
  it("locks after 5 fails, resets on success", () => {
    let t = 0;
    const th = createThrottle({ now: () => t });
    for (let i = 0; i < 5; i++) assert.equal(th.attempt("1.2.3.4", false).blocked, false);
    assert.equal(th.attempt("1.2.3.4", false).blocked, true); // locked out
    t += 31_000;
    assert.equal(th.blocked("1.2.3.4"), false); // lock expired
    t += 61_000; // old fails slide out of the window
    th.attempt("1.2.3.4", false);
    th.attempt("1.2.3.4", true); // success resets
    assert.equal(th.blocked("1.2.3.4"), false);
  });
});
