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
  slot2Text,
  STATE_WORDS,
} from "../src/format.js";
import type { SessionSnap, SessionState } from "../src/types.js";

const STATES: SessionState[] = [
  "idle",
  "thinking",
  "needs_input",
  "complete",
  "error",
  "ended",
  "unknown",
];

function mkSession(overrides: Partial<SessionSnap> = {}): SessionSnap {
  return {
    id: "abcdefgh1234",
    label: "",
    cwd: null,
    permissionMode: null,
    state: "idle",
    stateSinceMs: 0,
    detailKind: null,
    detailTool: null,
    question: null,
    options: [],
    error: null,
    children: 0,
    openOps: [],
    lastEventAtMs: 0,
    unreadSinceMs: null,
    pendingComplete: false,
    ...overrides,
  };
}

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

test("GLYPHS has a non-empty glyph for every SessionSnap state", () => {
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

// ---- slot2Text ------------------------------------------------------------

test("slot2Text: needs_input with detailKind question", () => {
  const s = mkSession({ state: "needs_input", detailKind: "question" });
  assert.equal(slot2Text(s), "question");
});

test("slot2Text: needs_input with detailKind permission", () => {
  const s = mkSession({ state: "needs_input", detailKind: "permission" });
  assert.equal(slot2Text(s), "permission");
});

test("slot2Text: needs_input with no detailKind", () => {
  const s = mkSession({ state: "needs_input", detailKind: null });
  assert.equal(slot2Text(s), "input needed");
});

test("slot2Text: error state with error detail present", () => {
  const s = mkSession({ state: "error", error: { kind: "tool_failed", message: null } });
  assert.equal(slot2Text(s), "tool_failed");
});

test("slot2Text: error state with no error detail falls back to the state word", () => {
  const s = mkSession({ state: "error", error: null });
  assert.equal(slot2Text(s), "error");
});

test("slot2Text: thinking with an open tool call shows the tool name", () => {
  const s = mkSession({
    state: "thinking",
    openOps: [{ id: "1", tool: "Bash", openedAtMs: 0 }],
  });
  assert.equal(slot2Text(s), "Bash");
});

test("slot2Text: thinking with no open tool call shows the state word", () => {
  const s = mkSession({ state: "thinking", openOps: [] });
  assert.equal(slot2Text(s), "thinking");
});

test("slot2Text: multiple open ops reports the newest (last) one", () => {
  const s = mkSession({
    state: "thinking",
    openOps: [
      { id: "1", tool: "Read", openedAtMs: 0 },
      { id: "2", tool: "Write", openedAtMs: 1 },
    ],
  });
  assert.equal(slot2Text(s), "Write");
});

test("slot2Text: detailTool is shown when there are no open ops", () => {
  const s = mkSession({ state: "complete", detailTool: "Grep" });
  assert.equal(slot2Text(s), "Grep");
});

test("slot2Text: idle with no detail falls back to the state word", () => {
  assert.equal(slot2Text(mkSession({ state: "idle" })), "idle");
});

test("slot2Text: complete with no detail falls back to the state word", () => {
  assert.equal(slot2Text(mkSession({ state: "complete" })), "complete");
});

test("slot2Text: ended with no detail falls back to the state word", () => {
  assert.equal(slot2Text(mkSession({ state: "ended" })), "ended");
});

test("slot2Text: unknown with no detail falls back to the state word", () => {
  assert.equal(slot2Text(mkSession({ state: "unknown" })), "unknown");
});
