// The Deckhand surface. It draws a list of sessions and takes pointer
// input, and nothing else: no authority, no inference, no keyboard
// handlers at all (docs/ARCHITECTURE.md#the-surface,
// docs/ACCESSIBILITY.md).
//
// One row per session, auto-bound by the daemon: there is no bind
// picker and no fixed row count. A click selects the row and raises its
// window in one motion (ADR-027); a raise that fails explains itself as
// a brief inline note on that row instead of opening anything.

import {
  displayName,
  escapeHtml,
  GLYPHS,
  greyLabel,
  isRevealSuccess,
  revealNote,
  rowLabel,
  STATE_WORDS,
  stateGlyph,
  stateWord,
  summaryCounts,
  unknownCount,
} from "./format.js";
import { presentSessionIds, staleNoteIds } from "./notes.js";
import { applyTheme, dark } from "./theme.js";
import { Snapshot, TileSnapshot, tauri } from "./types.js";

const api = tauri();

let snapshot: Snapshot = { tiles: [], nowMs: Date.now(), hideUnknown: false };
let lastActivityAt = Date.now();

const IDLE_DIM_MS = 3 * 60 * 1000;
const NOTE_MS = 4000;

// A row's inline note (a Reveal miss, shown for a few seconds), keyed by
// session id (PR review: identity). Indices shift as sessions bind and
// unbind; keying by id instead is what keeps a delayed miss pinned to
// the session it was raised for, even if rows above it come or go
// before the note is dropped. Pruned back to the present session set on
// every render (see pruneRowNotes) so a note never outlives its session.
const rowNotes = new Map<string, { text: string; timer: ReturnType<typeof setTimeout> }>();

const surface = document.getElementById("surface")!;
const list = document.getElementById("list")!;
const summary = document.getElementById("summary")!;
const grey = document.getElementById("grey")!;

// ---- Rows -------------------------------------------------------------

function renderRow(t: TileSnapshot): HTMLElement {
  const el = document.createElement("button");
  el.className = "row";
  if (t.selected) el.classList.add("selected");

  const s = t.session;
  if (!s) {
    // Defensive only: every bound row should resolve to a session.
    el.classList.add("row-empty");
    el.textContent = "…";
    el.setAttribute("aria-label", "Unbound session slot");
    return el;
  }

  el.dataset["state"] = s.state;
  const spinning = s.state === "thinking" ? " spinning" : "";
  const glyph = stateGlyph(s.state);

  el.innerHTML = `
    <div class="glyph${spinning}">${glyph}</div>
    <div class="row-name">${escapeHtml(displayName(s))}</div>
    <div class="row-state">${escapeHtml(stateWord(s))}</div>`;

  const note = rowNotes.get(s.id);
  if (note) {
    const noteEl = document.createElement("div");
    noteEl.className = "row-note";
    noteEl.textContent = note.text;
    el.append(noteEl);
  }
  // setAttribute takes a literal string, not an HTML fragment, so this
  // does not need escapeHtml the way the innerHTML above does; it is
  // built from the same displayName/stateWord facts either way, just
  // read by a screen reader instead of an eye.
  el.setAttribute("aria-label", rowLabel(s, note?.text));

  // One intent, one session id (PR review: identity): activate_session
  // resolves this id itself, at the moment the daemon actually looks,
  // rather than trusting the row index captured here to still name the
  // same session by the time either half of the old two-call sequence
  // ran. The closure below captures sessionId, not t.index, so the
  // result -- success or a delayed miss -- always lands on this
  // session's own note, never on whatever row it happens to occupy by
  // the time the reply arrives.
  const sessionId = s.id;
  el.addEventListener("click", () => {
    void api.core
      .invoke<string>("activate_session", { sessionId })
      .then((text) => {
        if (!isRevealSuccess(text)) showRowNote(sessionId, revealNote(text));
      })
      .catch(() => {
        // Defensive only: activate_session is designed to always
        // resolve (see main.rs), never reject. If it somehow does
        // anyway, the failure must still be visible on this row rather
        // than vanish as an unhandled promise rejection (PR review:
        // blocking, "an invoke rejection must not vanish silently").
        showRowNote(sessionId, "Reveal failed");
      });
  });
  return el;
}

function showRowNote(sessionId: string, text: string): void {
  const existing = rowNotes.get(sessionId);
  if (existing) clearTimeout(existing.timer);
  const timer = setTimeout(() => {
    rowNotes.delete(sessionId);
    render();
  }, NOTE_MS);
  rowNotes.set(sessionId, { text, timer });
  render();
}

// Drop every note whose session is no longer present, so a delayed
// result for a session that has since ended does not linger forever
// (it never renders once its session is gone, but the Map entry and
// its timer would otherwise outlive it for no reason).
function pruneRowNotes(): void {
  const present = presentSessionIds(snapshot.tiles);
  for (const id of staleNoteIds(rowNotes.keys(), present)) {
    const note = rowNotes.get(id);
    if (note) clearTimeout(note.timer);
    rowNotes.delete(id);
  }
}

// ---- Header counts ----------------------------------------------------

function renderSummary(): void {
  const states = snapshot.tiles.flatMap((t) => (t.session ? [t.session.state] : []));
  summary.replaceChildren(
    ...summaryCounts(states).map(([state, n]) => {
      const el = document.createElement("span");
      el.className = "count";
      el.dataset["state"] = state;
      el.setAttribute("aria-label", `${n} ${STATE_WORDS[state]}`);
      el.innerHTML = `<span class="glyph">${GLYPHS[state]}</span>${n}`;
      return el;
    }),
  );
}

// ---- Header grey toggle -----------------------------------------------

// Hides rows in the unknown state, either road in: not heard yet, or
// heard from and then silent past T_unknown (docs/ACCESSIBILITY.md: a
// single click, no hold, no keyboard). The label names what it hides.
// With nothing unknown and nothing hidden it has no job, so it steps out
// of the header; Quit, pinned to the right edge, never moves because of
// it.
function renderGrey(): void {
  const n = unknownCount(snapshot.tiles);
  grey.hidden = n === 0 && !snapshot.hideUnknown;
  grey.setAttribute("aria-pressed", String(snapshot.hideUnknown));
  grey.textContent = greyLabel(n, snapshot.hideUnknown);
}

function render(): void {
  pruneRowNotes();
  renderSummary();
  renderGrey();
  if (snapshot.tiles.length === 0) {
    const empty = document.createElement("div");
    empty.className = "row row-empty";
    empty.textContent = "Watching for sessions";
    list.replaceChildren(empty);
    return;
  }
  const visibleTiles = snapshot.hideUnknown
    ? snapshot.tiles.filter((t) => t.session?.state !== "unknown")
    : snapshot.tiles;
  if (snapshot.hideUnknown && visibleTiles.length === 0) {
    const hidden = document.createElement("div");
    hidden.className = "row row-empty";
    hidden.textContent = `${unknownCount(snapshot.tiles)} unknown hidden`;
    list.replaceChildren(hidden);
    return;
  }
  list.replaceChildren(...visibleTiles.map(renderRow));
}

// ---- Wiring -------------------------------------------------------------

function wake(): void {
  lastActivityAt = Date.now();
  surface.classList.remove("dimmed");
}

grey.addEventListener("click", () => {
  void api.core.invoke("toggle_hide_unknown");
});

document.getElementById("quit")!.addEventListener("click", () => {
  void api.core.invoke("quit");
});

document.addEventListener("pointermove", wake);
document.addEventListener("pointerdown", wake);

setInterval(() => {
  if (Date.now() - lastActivityAt > IDLE_DIM_MS) {
    surface.classList.add("dimmed");
  }
}, 1000);

async function init(): Promise<void> {
  applyTheme(dark);
  await api.event.listen<Snapshot>("deckhand://snapshot", (e) => {
    snapshot = e.payload;
    wake();
    render();
  });
  snapshot = await api.core.invoke<Snapshot>("snapshot");
  render();
}

void init();
