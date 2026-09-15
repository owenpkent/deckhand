// Pins the pure helpers in src/format.ts. These are the only DOM-free
// pieces of the surface, so they are the only pieces node:test can reach
// without a browser (docs/ARCHITECTURE.md#the-surface).

import assert from "node:assert/strict";
import test from "node:test";

import {
  boolChecked,
  countPillLabel,
  displayName,
  escapeHtml,
  fmtElapsed,
  gearIconKind,
  GLYPHS,
  hideGreyAriaLabel,
  hideGreyWord,
  hookStatusPillClass,
  hookStatusText,
  isRevealSuccess,
  onOffText,
  repairButtonInactive,
  repairSecondaryText,
  resetPositionText,
  revealNote,
  rowLabel,
  startWithWindowsChecked,
  startWithWindowsText,
  STATE_WORDS,
  stateGlyph,
  stateWord,
  summaryCounts,
  unknownCount,
} from "../src/format.js";
import type { HookStatus, RepairOutcome, SessionState, StartWithWindowsState } from "../src/types.js";

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

// ---- stateGlyph -----------------------------------------------------------

test("stateGlyph returns the matching glyph for every known state", () => {
  for (const state of STATES) {
    assert.equal(stateGlyph(state), GLYPHS[state]);
  }
});

test("stateGlyph falls back to the unknown glyph for a state outside the union", () => {
  // The daemon and surface can drift (a build mismatch, or a state one
  // side knows and the other doesn't yet); the wire only carries a
  // string, so this is reachable at runtime however TypeScript types it.
  const bogus = "reticulating" as unknown as SessionState;
  assert.equal(stateGlyph(bogus), GLYPHS.unknown);
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

// ---- rowLabel -------------------------------------------------------------

test("rowLabel joins the name and state word, with no note", () => {
  assert.equal(
    rowLabel({ id: "s1", label: "My Session", state: "thinking", heard: true }),
    "My Session, thinking"
  );
});

test("rowLabel appends the reveal note when one is showing", () => {
  assert.equal(
    rowLabel({ id: "s1", label: "My Session", state: "error", heard: true }, "No window found"),
    "My Session, error, No window found"
  );
});

test("rowLabel reflects not-heard-yet the same way stateWord does", () => {
  assert.equal(
    rowLabel({ id: "s1", label: "", state: "unknown", heard: false }),
    "s1, not heard yet"
  );
});

// ---- isRevealSuccess --------------------------------------------------------

test("isRevealSuccess is true for a raised-window sentence", () => {
  assert.equal(isRevealSuccess('Raised "deckhand - undertow".'), true);
});

test("isRevealSuccess is true for a VS Code lock-match raise, which has no window title to quote", () => {
  assert.equal(isRevealSuccess('Raised "deckhand" in VS Code.'), true);
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

// ---- summaryCounts ------------------------------------------------------------

test("summaryCounts keeps only non-zero header states, in display order", () => {
  const states: SessionState[] = ["thinking", "idle", "needs_input", "thinking", "ended", "complete"];
  assert.equal(
    JSON.stringify(summaryCounts(states)),
    JSON.stringify([
      ["needs_input", 1],
      ["thinking", 2],
      ["complete", 1],
    ])
  );
  assert.equal(summaryCounts([]).length, 0);
});

// ---- stateWord ------------------------------------------------------------------

test("stateWord says not heard yet only for an unknown session no hook has spoken for", () => {
  assert.equal(stateWord({ state: "unknown", heard: false }), "not heard yet");
  assert.equal(stateWord({ state: "unknown", heard: true }), "unknown");
  for (const state of STATES) {
    if (state === "unknown") continue;
    assert.equal(stateWord({ state, heard: false }), STATE_WORDS[state]);
  }
});

test("stateWord falls back to unknown for a state outside the union instead of returning undefined", () => {
  // Regression: STATE_WORDS[state] used to be indexed unguarded, so a
  // state the surface doesn't recognise returned undefined, and
  // escapeHtml(undefined) threw and blanked the whole session list.
  const bogus = "reticulating" as unknown as SessionState;
  assert.equal(stateWord({ state: bogus, heard: true }), "unknown");
  // Would throw before the fix (escapeHtml(undefined)); a plain call
  // that fails the test on a throw is the regression check.
  assert.equal(escapeHtml(stateWord({ state: bogus, heard: true })), "unknown");
});

// ---- unknownCount ---------------------------------------------------------

test("unknownCount counts only tiles whose session state is unknown", () => {
  assert.equal(
    unknownCount([
      { session: { state: "unknown" } },
      { session: { state: "idle" } },
      { session: { state: "unknown" } },
      { session: { state: "thinking" } },
    ]),
    2
  );
});

test("unknownCount is zero when there are no unknown tiles", () => {
  assert.equal(unknownCount([{ session: { state: "idle" } }, { session: { state: "complete" } }]), 0);
});

test("unknownCount is zero for an empty tile list", () => {
  assert.equal(unknownCount([]), 0);
});

test("unknownCount treats a null session (defensive-only row) as not unknown", () => {
  assert.equal(unknownCount([{ session: null }, { session: { state: "unknown" } }]), 1);
});

// ---- Header count pill (docs/DECISIONS.md#adr-034) -------------------------

test("countPillLabel names the count and the state word together", () => {
  assert.equal(countPillLabel("thinking", 2), "2 thinking");
  assert.equal(countPillLabel("needs_input", 1), "1 waiting on you");
  assert.equal(countPillLabel("complete", 0), "0 complete");
});

// ---- Settings panel helpers (docs/DECISIONS.md#adr-033, restyled by adr-034)

test("gearIconKind swaps to the back glyph only while the panel is open", () => {
  assert.equal(gearIconKind(false), "gear");
  assert.equal(gearIconKind(true), "back");
});

test("boolChecked renders a literal true/false string for aria-checked", () => {
  assert.equal(boolChecked(true), "true");
  assert.equal(boolChecked(false), "false");
});

test("onOffText is a plain On or Off", () => {
  assert.equal(onOffText(true), "On");
  assert.equal(onOffText(false), "Off");
});

test("startWithWindowsText covers off, on for this exe, and on for another copy", () => {
  const off: StartWithWindowsState = { kind: "off" };
  const onThisExe: StartWithWindowsState = { kind: "onThisExe" };
  const onOtherExe: StartWithWindowsState = { kind: "onOtherExe", path: "D:/old/deckhand.exe" };
  assert.equal(startWithWindowsText(off), "Off");
  assert.equal(startWithWindowsText(onThisExe), "On");
  assert.equal(startWithWindowsText(onOtherExe), "On (other copy)");
});

test("startWithWindowsChecked reads checked for both on-shaped kinds, unchecked only for off", () => {
  assert.equal(startWithWindowsChecked({ kind: "off" }), "false");
  assert.equal(startWithWindowsChecked({ kind: "onThisExe" }), "true");
  assert.equal(startWithWindowsChecked({ kind: "onOtherExe", path: "D:/old/deckhand.exe" }), "true");
});

test("hideGreyWord names the action off and the count on", () => {
  assert.equal(hideGreyWord(false, 6), "Hide grey");
  assert.equal(hideGreyWord(true, 6), "6 hidden");
  assert.equal(hideGreyWord(true, 0), "0 hidden");
});

test("hideGreyAriaLabel always names grey rows, with a count once hiding is on", () => {
  assert.equal(hideGreyAriaLabel(false, 6), "Hide grey rows");
  assert.equal(hideGreyAriaLabel(true, 6), "Hide grey rows, 6 hidden");
  assert.equal(hideGreyAriaLabel(true, 0), "Hide grey rows, 0 hidden");
});

test("hookStatusText renders every HookStatus as its own capitalised word", () => {
  const expected: Record<HookStatus, string> = {
    installed: "Installed",
    outdated: "Outdated",
    missing: "Missing",
    unreadable: "Unreadable",
  };
  for (const status of Object.keys(expected) as HookStatus[]) {
    assert.equal(hookStatusText(status), expected[status]);
  }
});

test("hookStatusPillClass tints installed good, outdated warn, and missing/unreadable bad", () => {
  assert.equal(hookStatusPillClass("installed"), "pill-good");
  assert.equal(hookStatusPillClass("outdated"), "pill-warn");
  assert.equal(hookStatusPillClass("missing"), "pill-bad");
  assert.equal(hookStatusPillClass("unreadable"), "pill-bad");
});

test("repairSecondaryText prioritises unavailable, then running, then the last outcome", () => {
  assert.equal(repairSecondaryText(false, false, null), "Installer not found");
  assert.equal(
    repairSecondaryText(false, true, "ran"),
    "Installer not found",
    "unavailable outranks a stale running/outcome state",
  );
  assert.equal(repairSecondaryText(true, true, null), "Running\u2026");
  assert.equal(
    repairSecondaryText(true, false, null),
    "",
    "idle and available has nothing to report; the button itself already reads Repair",
  );
});

test("repairSecondaryText names every RepairOutcome once it is available and idle", () => {
  const expected: Record<RepairOutcome, string> = {
    ran: "Repaired",
    timed_out: "Timed out",
    failed_to_start: "Failed to start",
  };
  for (const outcome of Object.keys(expected) as RepairOutcome[]) {
    assert.equal(repairSecondaryText(true, false, outcome), expected[outcome]);
  }
});

test("repairButtonInactive is true when there is no installer or a repair is already running", () => {
  assert.equal(repairButtonInactive(false, false), true);
  assert.equal(repairButtonInactive(true, true), true);
  assert.equal(repairButtonInactive(false, true), true);
  assert.equal(repairButtonInactive(true, false), false);
});

test("resetPositionText names the action, then confirms it briefly after a click", () => {
  assert.equal(resetPositionText(false), "Moves the window back to its default spot");
  assert.equal(resetPositionText(true), "Done");
});

// ---- revealNote -----------------------------------------------------------------

test("revealNote shortens each daemon miss sentence to one short line", () => {
  assert.equal(
    revealNote('No window matched "undertow". Reveal is a title and pid heuristic; the session may have no window on this machine.'),
    "No window found"
  );
  assert.equal(revealNote('Found "undertow" but Windows refused the raise.'), "Windows blocked it");
  assert.equal(revealNote("No session is bound to this row."), "No session");
  assert.equal(
    revealNote('Found "undertow" in Windows Terminal, but more than one Terminal window is open.'),
    "Multiple terminals"
  );
  assert.equal(
    revealNote('Found "deckhand" in VS Code, but more than one matching window is open.'),
    "Multiple VS Code windows"
  );
  assert.equal(revealNote("Reveal did not finish."), "Reveal failed");
  assert.equal(revealNote("Something new"), "Something new");
});
