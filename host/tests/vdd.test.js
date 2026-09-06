import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { addMonitor, removeMonitor, listMonitors, ensureMonitor, __resetStub, EDID_PROFILES } from "../src/vdd.js";

describe("vdd (stub registry; no driver on dev box)", () => {
  beforeEach(() => __resetStub());

  it("add/list/remove roundtrip", async () => {
    const a = await addMonitor({ width: 1280, height: 720, fps: 60 }, "phone-720p");
    assert.equal(a.ok, true);
    assert.equal(a.stub, true);
    const l = await listMonitors();
    assert.equal(l.monitors.length, 1);
    const r = await removeMonitor(a.monitor.id);
    assert.equal(r.ok, true);
    assert.equal((await listMonitors()).monitors.length, 0);
  });

  it("remove unknown id fails cleanly (departure idempotency)", async () => {
    const r = await removeMonitor(999);
    assert.equal(r.ok, false);
  });

  it("ensureMonitor reuses matching mode (idempotent extend)", async () => {
    const m = { width: 1920, height: 1080, fps: 60 };
    const first = await ensureMonitor(m, "phone-1080p");
    const second = await ensureMonitor(m, "phone-1080p");
    assert.equal(first.reused, false);
    assert.equal(second.reused, true);
    assert.equal(second.monitor.id, first.monitor.id);
    assert.equal((await listMonitors()).monitors.length, 1);
  });

  it("ships phone EDID profiles", () => {
    assert.deepEqual(EDID_PROFILES["phone-720p"], { width: 1280, height: 720, fps: 60 });
    assert.deepEqual(EDID_PROFILES["phone-1080p"], { width: 1920, height: 1080, fps: 60 });
  });
});
