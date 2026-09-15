// Pins the one fact worth pinning about icons.ts: every exported glyph
// is a real, non-empty inline svg, the same sanity check format.test.ts
// already runs for the session-state GLYPHS (docs/DECISIONS.md#adr-034).

import assert from "node:assert/strict";
import test from "node:test";

import { BACK_ICON, GEAR_ICON, QUIT_ICON, RESET_ICON } from "../src/icons.js";

const ICONS: Record<string, string> = { GEAR_ICON, BACK_ICON, QUIT_ICON, RESET_ICON };

test("every icon is a non-empty inline svg string", () => {
  for (const [name, svg] of Object.entries(ICONS)) {
    assert.equal(typeof svg, "string", `${name} is not a string`);
    assert.ok(svg.startsWith("<svg "), `${name} does not start with an <svg> tag`);
    assert.ok(svg.includes("</svg>"), `${name} is not a closed svg element`);
  }
});

test("the gear and its back glyph are two different shapes", () => {
  // gearIconKind (format.ts) picks between these two by name; this pins
  // that they actually differ, not just that both exist.
  assert.ok((GEAR_ICON as string) !== (BACK_ICON as string));
});
