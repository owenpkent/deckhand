// Pins two accessibility-critical facts about styles.css that live outside
// the type system (docs/ACCESSIBILITY.md, ADR-008): every tile state has a
// rule (colour is never the only channel), and the 44 px minimum hit
// target is actually present on the controls that need it.
//
// styles.css is not compiled, so it is read from disk relative to this
// compiled test file rather than imported.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const here = path.dirname(fileURLToPath(import.meta.url));
// dist-test/test -> dist-test -> app/ui, where styles.css actually lives.
const cssPath = path.join(here, "../../styles.css");
// Normalise line endings first: a checkout with autocrlf, which is what
// the GitHub Windows runners do, hands back CRLF, and the multi-line
// selectors matched below are written with plain newlines.
const css = readFileSync(cssPath, "utf8").replace(/\r\n/g, "\n");

// Returns the declaration body of the first rule whose selector text is
// `selector`. Requires selector to include enough of the following
// whitespace/brace to be unambiguous among near-neighbour selectors
// (e.g. ".picker-row {" not ".picker-row"). Good enough for this flat,
// unnested stylesheet.
function ruleBlock(selector: string): string {
  const needle = `${selector} {`;
  const idx = css.indexOf(needle);
  assert.ok(idx !== -1, `selector not found in styles.css: ${selector}`);
  const braceOpen = idx + needle.length - 1;
  const braceClose = css.indexOf("}", braceOpen);
  assert.ok(braceClose !== -1, `unterminated rule for: ${selector}`);
  return css.slice(braceOpen + 1, braceClose);
}

// ---- Every tile state carries a rule (colour is never the only channel) --

const TILE_STATES = [
  "idle",
  "thinking",
  "needs_input",
  "complete",
  "error",
  "unknown",
] as const;

test("styles.css has a [data-state] rule for every live tile state", () => {
  // main.ts sets el.dataset["state"] = s.state, so the selector is an
  // attribute selector, not a plain class.
  for (const state of TILE_STATES) {
    assert.ok(
      css.includes(`[data-state="${state}"]`),
      `missing rule for state: ${state}`
    );
  }
});

test("styles.css handles the ended state via the unbound rule", () => {
  // renderTile never sets data-state for an unbound tile, and treats
  // "ended" the same as unbound in the stylesheet (a dashed, colourless
  // border): pin that shared rule explicitly instead of assuming it
  // matches the pattern above.
  const block = ruleBlock('.tile[data-state="ended"],\n.tile.unbound');
  assert.match(block, /border:\s*3px dashed/);
});

// ---- 44 px minimum hit target (docs/ACCESSIBILITY.md) --------------------

test("the tile itself is at least the 44px floor", () => {
  const block = ruleBlock(".tile");
  const width = /width:\s*(\d+)px/.exec(block);
  const height = /height:\s*(\d+)px/.exec(block);
  assert.ok(width, "tile has no explicit width");
  assert.ok(height, "tile has no explicit height");
  assert.ok(Number(width![1]) >= 44, "tile width below the 44px floor");
  assert.ok(Number(height![1]) >= 44, "tile height below the 44px floor");
});

test("the command key grid rows meet the 44px floor", () => {
  const block = ruleBlock("#keys");
  assert.match(block, /grid-template-rows:\s*repeat\(2,\s*44px\)/);
});

test("the stick pad cells meet the 44px floor", () => {
  const block = ruleBlock("#stick");
  assert.match(block, /grid-template-columns:\s*repeat\(2,\s*44px\)/);
  assert.match(block, /grid-template-rows:\s*repeat\(2,\s*44px\)/);
});

test("the dial commit button meets the 44px floor", () => {
  const block = ruleBlock("#dial .dial-commit");
  assert.match(block, /width:\s*44px/);
  assert.match(block, /height:\s*44px/);
});

test("the dial steppers meet the 44px floor", () => {
  const block = ruleBlock("#dial .dial-steppers .ctl");
  assert.match(block, /height:\s*44px/);
});

test("the talk column controls meet the 44px floor", () => {
  const block = ruleBlock("#talkcol .ctl");
  assert.match(block, /height:\s*44px/);
});

test("the side buttons (move, quit) meet the 44px floor", () => {
  const block = ruleBlock(".side-btn");
  assert.match(block, /min-height:\s*44px/);
});

test("answer-target buttons in the panel meet the 44px floor", () => {
  const block = ruleBlock(".answer-targets .ctl");
  assert.match(block, /min-height:\s*44px/);
});

test("panel action buttons (Reveal, Unbind, Scan) meet the 44px floor", () => {
  const block = ruleBlock(".panel-actions .ctl");
  assert.match(block, /min-height:\s*44px/);
});

test("bind picker rows meet the 44px floor", () => {
  const block = ruleBlock(".picker-row");
  assert.match(block, /min-height:\s*44px/);
});

test("bind picker buttons meet the 44px floor", () => {
  const block = ruleBlock(".picker-btn");
  assert.match(block, /min-width:\s*44px/);
});
