import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { signature, watchInterfaces } from "../src/iface.js";

describe("iface watcher", () => {
  it("signature is order-insensitive", () => {
    assert.equal(signature(["b", "a"]), signature(["a", "b"]));
    assert.notEqual(signature(["a"]), signature(["a", "b"]));
  });

  it("check() fires once per change (USB replug)", () => {
    let addrs = ["192.168.1.10"];
    let fires = 0;
    const w = watchInterfaces({ intervalMs: 60_000, list: () => addrs, onChange: () => { fires += 1; } });
    assert.equal(w.check(), false);
    addrs = ["192.168.1.10", "192.168.42.129"]; // tethering plugged
    assert.equal(w.check(), true);
    assert.equal(w.check(), false);
    addrs = ["192.168.1.10"]; // unplugged
    assert.equal(w.check(), true);
    assert.equal(fires, 2);
    w.stop();
  });
});
