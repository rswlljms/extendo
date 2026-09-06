import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { createHost } from "../src/server.js";
import { defaultConfig } from "../src/config.js";
import { __resetStub } from "../src/vdd.js";

process.env.EXTENDO_INPUT_DRY = "1"; // never spawn the PS runner in tests

describe("host HTTP contract", () => {
  const cfg = { ...defaultConfig(), port: 0, video: { width: 1280, height: 720, fps: 30, bitrate_kbps: 4000, codec: "h264" } };
  const { server } = createHost(cfg);
  let base = "";
  const H = () => ({ "Content-Type": "application/json", "x-extendo-token": cfg.token });

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

  it("POST /pair throttles brute force (429 after 5 fails)", async () => {
    await fetch(`${base}/pair`, { // reset counter with a good attempt
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device_name: "t", token: cfg.token }),
    });
    for (let i = 0; i < 5; i++) {
      const r = await fetch(`${base}/pair`, {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ device_name: "x", token: "wrong" }),
      });
      assert.equal(r.status, 403);
    }
    const locked = await fetch(`${base}/pair`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ device_name: "x", token: "wrong" }),
    });
    assert.equal(locked.status, 429);
  });

  it("POST /quality gates 1080p60 on free tier", async () => {
    const r = await fetch(`${base}/quality`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ width: 1920, height: 1080, fps: 60 }),
    });
    assert.equal(r.status, 402);
    const j = await r.json();
    assert.equal(j.tier, "free");
  });

  it("mutation endpoints require token (401 without)", async () => {
    for (const [m, p] of [["POST", "/quality"], ["POST", "/input"], ["POST", "/capture"], ["GET", "/displays"]]) {
      const r = await fetch(`${base}${p}`, { method: m, headers: { "Content-Type": "application/json" }, body: m === "GET" ? undefined : "{}" });
      assert.equal(r.status, 401, `${m} ${p}`);
    }
  });

  it("POST /input routes mouse (free) but gates touch (Pro)", async () => {
    const mouse = await fetch(`${base}/input`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ type: "mouse_move", dx: 3, dy: 0 }),
    });
    assert.equal(mouse.status, 200);
    const touch = await fetch(`${base}/input`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ type: "touch", x: 0.5, y: 0.5, down: true }),
    });
    assert.equal(touch.status, 402);
    const bad = await fetch(`${base}/input`, {
      method: "POST", headers: H(), body: JSON.stringify({ nope: 1 }),
    });
    assert.equal(bad.status, 400);
  });

  it("POST /report stores viewer stats for adaptive", async () => {
    const r = await fetch(`${base}/report`, {
      method: "POST", headers: H(), body: JSON.stringify({ fps: 30, drops: 0 }),
    });
    assert.equal(r.status, 200);
    assert.equal((await r.json()).adaptive, "high");
  });
});

describe("display endpoints (Phase 1 extend)", () => {
  const cfg = { ...defaultConfig(), port: 0, video: { width: 1280, height: 720, fps: 30, bitrate_kbps: 4000, codec: "h264" } };
  const { server } = createHost(cfg);
  let base = "";
  const H = () => ({ "Content-Type": "application/json", "x-extendo-token": cfg.token });

  before(() => new Promise((resolve) => {
    __resetStub();
    server.listen(0, "127.0.0.1", () => {
      base = `http://127.0.0.1:${server.address().port}`;
      resolve();
    });
  }));
  after(() => new Promise((resolve) => server.close(resolve)));

  it("POST /displays extends a 720p monitor and syncs video", async () => {
    const r = await fetch(`${base}/displays`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ mode: { width: 1280, height: 720, fps: 30 }, edid: "phone-720p" }),
    });
    assert.equal(r.status, 200);
    const j = await r.json();
    assert.equal(j.monitor.width, 1280);
    const info = await (await fetch(`${base}/info`)).json();
    assert.equal(info.video.width, 1280);
    assert.ok(Array.isArray(info.displays));
  });

  it("POST /displays gates 1080p60 on free tier", async () => {
    const r = await fetch(`${base}/displays`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ mode: { width: 1920, height: 1080, fps: 60 } }),
    });
    assert.equal(r.status, 402);
  });

  it("DELETE /displays/:id departs the monitor", async () => {
    const added = await (await fetch(`${base}/displays`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ mode: { width: 1280, height: 720, fps: 30 } }),
    })).json();
    const del = await fetch(`${base}/displays/${added.monitor.id}`, { method: "DELETE", headers: H() });
    assert.equal(del.status, 200);
  });

  it("POST /capture binds source to virtual monitor", async () => {
    const r = await fetch(`${base}/capture`, {
      method: "POST", headers: H(),
      body: JSON.stringify({ backend: "test", monitor_id: 1 }),
    });
    assert.equal(r.status, 200);
    assert.equal((await r.json()).source.monitor_id, 1);
  });
});
