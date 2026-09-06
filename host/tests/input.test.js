import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { toAbsolute, mapEvent, createInput } from "../src/input.js";

process.env.EXTENDO_INPUT_DRY = "1";

describe("input mapping", () => {
  it("normalizes to absolute 0..65535 and clamps", () => {
    assert.equal(toAbsolute(0), 0);
    assert.equal(toAbsolute(1), 65535);
    assert.equal(toAbsolute(0.5), 32768);
    assert.equal(toAbsolute(9), 65535);
    assert.equal(toAbsolute(-2), 0);
  });

  it("touch down/move/up sequence", () => {
    assert.deepEqual(mapEvent({ type: "touch", x: 0.5, y: 0.5, down: true }),
      [{ a: "move", x: 32768, y: 32768 }, { a: "left", down: true }]);
    assert.deepEqual(mapEvent({ type: "touch", x: 0.1, y: 0.2 }),
      [{ a: "move", x: 6554, y: 13107 }]);
    assert.deepEqual(mapEvent({ type: "touch", down: false }), [{ a: "left", down: false }]);
  });

  it("mouse absolute + relative, buttons, wheel, keys", () => {
    assert.deepEqual(mapEvent({ type: "mouse_move", x: 0, y: 1 }),
      [{ a: "move", x: 0, y: 65535 }]);
    assert.deepEqual(mapEvent({ type: "mouse_move", dx: 5, dy: -3 }), [{ a: "rel", dx: 5, dy: -3 }]);
    assert.deepEqual(mapEvent({ type: "mouse_down" }), [{ a: "left", down: true }]);
    assert.deepEqual(mapEvent({ type: "scroll", dy: 10 }), [{ a: "wheel", d: -120 }]);
    assert.deepEqual(mapEvent({ type: "key", key_code: 65, down: true }), [{ a: "key", vk: 65, down: true }]);
    assert.deepEqual(mapEvent({ type: "key" }), []);
    assert.deepEqual(mapEvent({ type: "nope" }), []);
  });

  it("dry-run executor counts without spawning", () => {
    const inp = createInput();
    assert.equal(inp.state.dry, true);
    const r = inp.handle({ type: "touch", x: 0.5, y: 0.5, down: true });
    assert.equal(r.ok, true);
    assert.equal(r.actions, 2);
    assert.equal(inp.handle(null).ok, false);
    assert.equal(inp.handle({ type: "touch" }).reason, "unmapped");
    inp.stop();
  });
});
