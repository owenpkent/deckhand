// Pins the pure helpers in src/format.ts. These are the only DOM-free
// pieces of the surface, so they are the only pieces node:test can reach
// without a browser (docs/ARCHITECTURE.md#the-surface).

import assert from "node:assert/strict";
import test from "node:test";

import {
  displayName,
  escapeHtml,
  fmtElapsed,
  GLYPHS,
  isRevealSuccess,
  STATE_WORDS,
} from "../src/format.js";
import type { SessionState } from "../src/types.js";

const STATES: SessionState[] = [
  "idle",
  "thinking",
  "needs_input",
  "complete",
  "error",
  "ended",
  "unknown",
];

// ---- escapeHtml -------------------------------------------------------

test("escapeHtml escapes & < > and leaves quotes untouched", () => {
  // The function only replaces &, < and > (see src/format.ts). Quotes pass
  // through unchanged; pinning that so a future "helpful" widening shows
  // up as a failing test rather than a silent behaviour change.
  assert.equal(escapeHtml(`&<>"'`), `&amp;&lt;&gt;"'`);
});

test("escapeHtml handles a plain string with nothing to escape", () => {
  assert.equal(escapeHtml("plain text"), "plain text");
});

test("escapeHtml escapes repeated characters", () => {
  assert.equal(escapeHtml("<<&&>>"), "&lt;&lt;&amp;&amp;&gt;&gt;");
});

// ---- fmtElapsed --------------------------------------------------------

test("fmtElapsed at 0 seconds", () => {
  assert.equal(fmtElapsed(1000, 1000), "0:00");
});

test("fmtElapsed clamps a negative delta (future fromMs) to 0", () => {
  assert.equal(fmtElapsed(2000, 1000), "0:00");
});

test("fmtElapsed at 59 seconds, just under the minute boundary", () => {
  assert.equal(fmtElapsed(0, 59_000), "0:59");
});

test("fmtElapsed at exactly 60 seconds rolls to 1:00", () => {
  assert.equal(fmtElapsed(0, 60_000), "1:00");
});

test("fmtElapsed at 59:59, just under the hour boundary", () => {
  assert.equal(fmtElapsed(0, 3_599_000), "59:59");
});

test("fmtElapsed at exactly one hour switches to the h-format", () => {
  // Note the hour branch drops seconds entirely: it is hours + remainder
  // minutes only, never minutes:seconds once past 60 minutes.
  assert.equal(fmtElapsed(0, 3_600_000), "1h00");
});

test("fmtElapsed at one hour, one minute, one second", () => {
  assert.equal(fmtElapsed(0, 3_661_000), "1h01");
});

test("fmtElapsed at two hours exactly", () => {
  assert.equal(fmtElapsed(0, 7_200_000), "2h00");
});

// ---- GLYPHS -------------------------------------------------------------

test("GLYPHS has a non-empty glyph for every session state", () => {
  for (const state of STATES) {
    const glyph = GLYPHS[state];
    assert.equal(typeof glyph, "string", `missing glyph for ${state}`);
    assert.ok(glyph && glyph.length > 0, `empty glyph for ${state}`);
  }
});

// ---- STATE_WORDS ---------------------------------------------------------

test("STATE_WORDS renders the expected word for every state", () => {
  const expected: Record<SessionState, string> = {
    idle: "idle",
    thinking: "thinking",
    needs_input: "waiting on you",
    complete: "complete",
    error: "error",
    ended: "ended",
    unknown: "unknown",
  };
  for (const state of STATES) {
    assert.equal(STATE_WORDS[state], expected[state]);
  }
});

// ---- displayName ----------------------------------------------------------

test("displayName prefers the label when present", () => {
  assert.equal(displayName({ id: "session-id-long", label: "My Session" }), "My Session");
});

test("displayName falls back to the first 8 chars of id when label is empty", () => {
  assert.equal(displayName({ id: "session-id-long", label: "" }), "session-");
});

// ---- isRevealSuccess --------------------------------------------------------

test("isRevealSuccess is true for a raised-window sentence", () => {
  assert.equal(isRevealSuccess('Raised "deckhand - undertow".'), true);
});

test("isRevealSuccess is false for a no-match miss", () => {
  assert.equal(
    isRevealSuccess('No window matched "undertow". Reveal is a title and pid heuristic; the session may have no window on this machine.'),
    false
  );
});

test("isRevealSuccess is false for a found-but-refused miss", () => {
  assert.equal(isRevealSuccess('Found "undertow" but Windows refused the raise.'), false);
});

test("isRevealSuccess is false for an unbound row", () => {
  assert.equal(isRevealSuccess("No session is bound to this row."), false);
});
