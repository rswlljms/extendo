import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { selectSource, currentSource, syncVideoToMode, coreAvailable } from "../src/capture.js";

describe("capture source", () => {
  it("defaults to test backend on virtual-mirror fallback", () => {
    const s = currentSource();
    assert.equal(s.backend, "test");
  });

  it("binds to a virtual monitor id", () => {
    const r = selectSource({ monitor_id: 1 });
    assert.equal(r.ok, true);
    assert.equal(currentSource().monitor_id, 1);
  });

  it("rejects unknown backends; wgc/dxgi need Rust core when not built", () => {
    assert.equal(selectSource({ backend: "nope" }).ok, false);
    if (coreAvailable()) {
      // Core is built (cargo build --release) -> wgc should be selectable.
      const r = selectSource({ backend: "wgc" });
      assert.equal(r.ok, true);
      assert.equal(r.source.backend, "wgc");
      // dxgi still not implemented.
      assert.match(selectSource({ backend: "dxgi" }).reason, /Rust core/);
      // reset to test for other suites
      selectSource({ backend: "test" });
    } else {
      assert.match(selectSource({ backend: "wgc" }).reason, /Rust core/);
    }
  });

  it("syncs VideoConfig to monitor mode", () => {
    const v = { width: 1280, height: 720, fps: 30 };
    syncVideoToMode(v, { width: 1920, height: 1080, fps: 60 });
    assert.deepEqual(v, { width: 1920, height: 1080, fps: 60 });
  });
});
