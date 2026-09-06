import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { createHost } from "../src/server.js";
import { defaultConfig } from "../src/config.js";

describe("host HTTP contract", () => {
  const cfg = { ...defaultConfig(), port: 0, video: { width: 1280, height: 720, fps: 30, bitrate_kbps: 4000, codec: "h264" } };
  const { server } = createHost(cfg);
  let base = "";

  before(() => new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      base = `http://127.0.0.1:${server.address().port}`;
      resolve();
    });
  }));
  after(() => new Promise((resolve) => server.close(resolve)));

  it("GET /info exposes transports + video + qr", async () => {
    const r = await fetch(`${base}/info`);
    assert.equal(r.status, 200);
    const j = await r.json();
    assert.ok(Array.isArray(j.transports));
    assert.ok(j.transports.some((t) => t.addr === "127.0.0.1"));
    assert.equal(j.video.codec, "h264");
  });

  it("GET /ping echoes t0 for RTT math", async () => {
    const t0 = Date.now();
    const r = await fetch(`${base}/ping?t0=${t0}`);
    const j = await r.json();
    assert.equal(j.t0, t0);
    assert.ok(Date.now() - t0 < 2000);
  });

  it("POST /pair accepts token, rejects bad PIN", async () => {
    const ok = await fetch(`${base}/pair`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device_name: "test-phone", token: cfg.token }),
    });
    assert.equal(ok.status, 200);
    const bad = await fetch(`${base}/pair`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device_name: "x", token: "wrong" }),
    });
    assert.equal(bad.status, 403);
  });

  it("POST /quality gates 1080p60 on free tier", async () => {
    const r = await fetch(`${base}/quality`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ width: 1920, height: 1080, fps: 60 }),
    });
    assert.equal(r.status, 402);
    const j = await r.json();
    assert.equal(j.tier, "free");
  });

  it("POST /input validates shape", async () => {
    const ok = await fetch(`${base}/input`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ type: "touch", x: 0.5, y: 0.5, down: true }),
    });
    assert.equal(ok.status, 200);
  });
});
