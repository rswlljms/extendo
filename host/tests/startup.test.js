import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// Guards the entry point: duplicate imports in index.js once crashed startup
// while all contract tests stayed green. --check is parse-only, no side effects.
describe("startup syntax (node --check)", () => {
  const src = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
  for (const f of readdirSync(src).filter((x) => x.endsWith(".js"))) {
    it(`${f} parses`, () => {
      execFileSync(process.execPath, ["--check", join(src, f)], { stdio: "pipe" });
      assert.ok(true);
    });
  }
});
