// Pure, DOM-free helpers shared by the surface. Nothing here touches the
// document, Tauri, or module-level mutable state, so these are safe to
// unit-test without a browser (docs/ARCHITECTURE.md#the-surface).

import { SessionState } from "./types.js";

// ---- Glyphs: drawn, never emoji (docs/UI_SPEC.md#state-rendering) ----

export const GLYPHS: Record<SessionState, string> = {
  idle: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="8"/></svg>`,
  thinking: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M12 4 a8 8 0 0 1 8 8"/></svg>`,
  needs_input: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 11V6a1.5 1.5 0 0 1 3 0v4V5a1.5 1.5 0 0 1 3 0v5V6.5a1.5 1.5 0 0 1 3 0V12v-2a1.5 1.5 0 0 1 3 0v5a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-2.7L4.6 14a1.6 1.6 0 0 1 2.6-1.8L8.5 14"/></svg>`,
  complete: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4 10-10"/></svg>`,
  error: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M6 6l12 12M18 6L6 18"/></svg>`,
  unknown: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M9 9a3 3 0 1 1 4.2 2.8c-.9.4-1.2 1-1.2 2.2"/><circle cx="12" cy="18" r="0.5" fill="currentColor"/></svg>`,
  ended: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 12h10"/></svg>`,
};

export const STATE_WORDS: Record<SessionState, string> = {
  idle: "idle",
  thinking: "thinking",
  needs_input: "waiting on you",
  complete: "complete",
  error: "error",
  ended: "ended",
  unknown: "unknown",
};

// Whether `state` is one of the SessionState variants STATE_WORDS and
// GLYPHS actually have an entry for. TypeScript trusts the SessionState
// annotation at every call site, but a session's state arrives over IPC
// from the daemon: version skew, or a state one side knows and the other
// doesn't yet, can hand the surface a string outside that union at
// runtime regardless of what the compiler believes. Guarding the lookup
// here is what stateWord/stateGlyph fall back on.
function isKnownState(state: string): state is SessionState {
  return Object.prototype.hasOwnProperty.call(STATE_WORDS, state);
}

// The word a row shows. Unknown has two roads in and says which one it
// took: a session bound at startup that no hook has spoken for yet is
// "not heard yet"; one that went silent past T_unknown stays "unknown".
// Same state, same colour, same glyph (ADR-008). A state outside the
// known set (see isKnownState) also reads as "unknown" rather than
// returning undefined: an unguarded lookup used to reach escapeHtml and
// throw, blanking the whole list.
export function stateWord(s: { state: SessionState; heard: boolean }): string {
  const state = isKnownState(s.state) ? s.state : "unknown";
  return state === "unknown" && !s.heard ? "not heard yet" : STATE_WORDS[state];
}

// The glyph for a session's state, with the same unknown-state fallback
// as stateWord and for the same reason.
export function stateGlyph(state: SessionState): string {
  return GLYPHS[isKnownState(state) ? state : "unknown"];
}

// The states the header counts, in display order: what needs a human
// first, then what is running, then what finished. Idle, unknown, and
// ended are left to the rows.
export const SUMMARY_STATES: readonly SessionState[] = ["needs_input", "error", "thinking", "complete"];

// Non-zero counts of SUMMARY_STATES among `states`, in SUMMARY_STATES
// order.
export function summaryCounts(states: readonly SessionState[]): [SessionState, number][] {
  return SUMMARY_STATES.map((s): [SessionState, number] => [s, states.filter((x) => x === s).length]).filter(
    ([, n]) => n > 0,
  );
}

// Count of bound rows currently in the unknown state, either road in
// (never heard from, or heard from and then silent past T_unknown).
// Feeds the grey toggle's label and its all-hidden placeholder; takes
// the minimal per-tile shape rather than the full TileSnapshot so it
// stays DOM-free and easy to test.
export function unknownCount(tiles: readonly { session: { state: SessionState } | null }[]): number {
  return tiles.filter((t) => t.session?.state === "unknown").length;
}

// The grey toggle's label, naming what it acts on: "Hide unknown" while
// those rows show, "Show 6 unknown" while they are hidden, so a hidden
// session is always counted.
export function greyLabel(unknown: number, hidden: boolean): string {
  if (!hidden) return "Hide unknown";
  return unknown === 0 ? "Show unknown" : `Show ${unknown} unknown`;
}

// A Reveal miss, cut down to what fits beside the state word. The
// daemon's full sentence explains the heuristic; the row only needs to
// say what happened.
export function revealNote(text: string): string {
  if (text.startsWith("No window matched")) return "No window found";
  if (text.includes("refused the raise")) return "Windows blocked it";
  if (text.startsWith("No session is bound")) return "No session";
  if (text.includes("more than one Terminal window")) return "Multiple terminals";
  if (text.includes("more than one matching window")) return "Multiple VS Code windows";
  // The reveal worker (reveal_queue.rs) never answered: it panicked, its
  // reply channel was dropped, or it was already gone. Same short label
  // as the invoke-rejection fallback in main.ts's own catch handler,
  // since both mean the same thing to whoever is looking at the row.
  if (text.startsWith("Reveal did not finish")) return "Reveal failed";
  return text;
}

// ---- Small helpers --------------------------------------------------

export function fmtElapsed(fromMs: number, nowMs: number): string {
  const s = Math.max(0, Math.floor((nowMs - fromMs) / 1000));
  const m = Math.floor(s / 60);
  if (m >= 60) {
    return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
  }
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

export function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

// Shared by session rows: prefer the human label, fall back to a short
// id fragment.
export function displayName(s: { id: string; label: string }): string {
  return s.label || s.id.slice(0, 8);
}

// A row button's accessible name. The row is a <button>, so with no
// aria-label the browser would compute one from its own text content
// (name, then state word) and get this for free; the point of building
// it explicitly is to fold in the Reveal-miss note when one is showing,
// since that note is a sibling element added after the fact and a
// generic "Session N" label was masking all of it, name and state word
// included, from anyone not reading the screen.
export function rowLabel(s: { id: string; label: string; state: SessionState; heard: boolean }, note?: string): string {
  const parts = [displayName(s), stateWord(s)];
  if (note) parts.push(note);
  return parts.join(", ");
}

// Reveal always returns a sentence, success or miss. A successful raise
// is visible on its own (the host window comes forward), so only a miss
// is worth showing as a row note; this is the plain-data decision behind
// that, pulled out so it is testable without touching the DOM.
export function isRevealSuccess(text: string): boolean {
  return text.startsWith("Raised ");
}
