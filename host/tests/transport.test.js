import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { isRndisIp, isPrivateIp, listCandidates, pickBest } from "../src/transport.js";

describe("transport", () => {
  it("flags RNDIS tethering ranges", () => {
    assert.equal(isRndisIp("192.168.42.129"), true);
    assert.equal(isRndisIp("192.168.43.1"), true);
    assert.equal(isRndisIp("192.168.137.1"), true);
    assert.equal(isRndisIp("192.168.1.10"), false);
  });

  it("accepts all private ranges incl. 10.x and 172.16-31.x", () => {
    assert.equal(isPrivateIp("10.0.0.5"), true);
    assert.equal(isPrivateIp("172.20.3.4"), true);
    assert.equal(isPrivateIp("172.32.0.1"), false);
    assert.equal(isPrivateIp("8.8.8.8"), false);
  });

  it("always includes localhost (ADB) candidate", () => {
    const c = listCandidates(9577, ["192.168.1.10"]);
    assert.ok(c.some((t) => t.kind === "localhost" && t.addr === "127.0.0.1"));
    assert.ok(c.some((t) => t.kind === "wifi" && t.addr === "192.168.1.10"));
  });

  it("labels rndis addrs correctly", () => {
    const c = listCandidates(9577, ["192.168.42.129"]);
    assert.equal(c.find((t) => t.addr === "192.168.42.129").kind, "rndis");
  });

  it("pickBest prefers lowest RTT, tie-break localhost > rndis > wifi", () => {
    const best = pickBest([
      { kind: "wifi", addr: "192.168.1.10", port: 9577, rttMs: 25 },
      { kind: "rndis", addr: "192.168.42.129", port: 9577, rttMs: 8 },
      { kind: "localhost", addr: "127.0.0.1", port: 9577, rttMs: 8 },
    ]);
    assert.equal(best.addr, "127.0.0.1");
    const wifiOnly = pickBest([
      { kind: "wifi", addr: "192.168.1.10", port: 9577, rttMs: 60 },
    ]);
    assert.equal(wifiOnly.addr, "192.168.1.10");
  });
});
