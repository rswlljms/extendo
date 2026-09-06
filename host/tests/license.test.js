import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { canUse, validateLicense, maxVideoForTier } from "../src/license.js";

describe("license (entitlement stub)", () => {
  it("free tier: wifi allowed, usb/1080p60 denied", () => {
    assert.equal(canUse("wifi", "free"), true);
    assert.equal(canUse("usb", "free"), false);
    assert.equal(canUse("1080p60", "free"), false);
  });

  it("pro tier unlocks usb + 1080p60", () => {
    assert.equal(canUse("usb", "pro"), true);
    assert.equal(canUse("1080p60", "pro"), true);
  });

  it("defaults unknown/empty to free offline", () => {
    assert.deepEqual(validateLicense({}), { tier: "free", offline: true, grace: false });
    assert.equal(maxVideoForTier("free").height, 720);
    assert.equal(maxVideoForTier("pro").height, 1080);
  });
});
