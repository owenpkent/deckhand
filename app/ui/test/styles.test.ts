// Pins two accessibility-critical facts about styles.css that live outside
// the type system (docs/ACCESSIBILITY.md, ADR-008): every session state has
// a rule (colour is never the only channel), and the 44 px minimum hit
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
// (e.g. ".row-name {" not ".row"). Good enough for this flat, unnested
// stylesheet.
function ruleBlock(selector: string): string {
  const needle = `${selector} {`;
  const idx = css.indexOf(needle);
  assert.ok(idx !== -1, `selector not found in styles.css: ${selector}`);
  const braceOpen = idx + needle.length - 1;
  const braceClose = css.indexOf("}", braceOpen);
  assert.ok(braceClose !== -1, `unterminated rule for: ${selector}`);
  return css.slice(braceOpen + 1, braceClose);
}

// ---- Every session state carries a rule (colour is never the only channel) --

const SESSION_STATES = [
  "idle",
  "thinking",
  "needs_input",
  "complete",
  "error",
  "unknown",
  "ended",
] as const;

test("styles.css has a row [data-state] rule for every session state", () => {
  // main.ts sets el.dataset["state"] = s.state, so the selector is an
  // attribute selector, not a plain class.
  for (const state of SESSION_STATES) {
    assert.ok(
      css.includes(`.row[data-state="${state}"]`),
      `missing rule for state: ${state}`
    );
  }
});

test("unknown and ended rows step back from live rows by more than colour", () => {
  // Their glyphs differ in shape (question mark, dash); on top of that the
  // name drops from bold, so a grey row reads apart from a live one even
  // in greyscale.
  assert.match(ruleBlock('.row[data-state="unknown"] .row-name'), /font-weight:\s*400/);
  assert.match(ruleBlock('.row[data-state="ended"] .glyph,\n.row[data-state="ended"] .row-name,\n.row[data-state="ended"] .row-state'), /color:/);
});

test("the header is one drag region and its counts pass drags through", () => {
  const html = readFileSync(path.join(here, "../../index.html"), "utf8");
  assert.match(html, /<div id="header" data-tauri-drag-region>/);
  assert.match(ruleBlock("#summary"), /pointer-events:\s*none/);
});

test("row and header heights match ROW_H_LOGICAL and HEADER_H_LOGICAL in window.rs", () => {
  // The daemon sizes the window from those constants; a mismatch spills
  // the list into a scrollbar or leaves dead space under the last row.
  const rs = readFileSync(path.join(here, "../../../src-tauri/src/window.rs"), "utf8");
  const rowRs = /ROW_H_LOGICAL: f64 = (\d+)/.exec(rs);
  const headerRs = /HEADER_H_LOGICAL: f64 = (\d+)/.exec(rs);
  assert.ok(rowRs && headerRs, "constants not found in window.rs");
  assert.match(ruleBlock(".row"), new RegExp(`min-height:\\s*${rowRs![1]}px`));
  assert.match(ruleBlock("#header"), new RegExp(`height:\\s*${headerRs![1]}px`));
});

// ---- 44 px minimum hit target (docs/ACCESSIBILITY.md) --------------------

test("a session row is at least the 44px floor", () => {
  const block = ruleBlock(".row");
  const minHeight = /min-height:\s*(\d+)px/.exec(block);
  assert.ok(minHeight, "row has no explicit min-height");
  assert.ok(Number(minHeight![1]) >= 44, "row min-height below the 44px floor");
});

test("the empty-list placeholder row is also at least the 44px floor", () => {
  const block = ruleBlock(".row-empty");
  const minHeight = /min-height:\s*(\d+)px/.exec(block);
  assert.ok(minHeight, "row-empty has no explicit min-height");
  assert.ok(Number(minHeight![1]) >= 44, "row-empty min-height below the 44px floor");
});

test("the header buttons (gear, Quit) meet the 44px floor", () => {
  const block = ruleBlock(".side-btn");
  assert.match(block, /min-height:\s*44px/);
});
