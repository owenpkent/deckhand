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
  STATE_WORDS,
  stateWord,
  summaryCounts,
  unknownCount,
} from "./format.js";
import { applyTheme, dark } from "./theme.js";
import { Snapshot, TileSnapshot, tauri } from "./types.js";

const api = tauri();

let snapshot: Snapshot = { tiles: [], nowMs: Date.now(), hideUnknown: false };
let lastActivityAt = Date.now();

const IDLE_DIM_MS = 3 * 60 * 1000;
const NOTE_MS = 4000;

// A row's inline note (a Reveal miss, shown for a few seconds), keyed by
// row index. Indices shift as sessions bind and unbind, so a note is
// scoped to "whatever is in this row right now", not to a session id;
// that matches how briefly it is ever shown.
const rowNotes = new Map<number, { text: string; timer: ReturnType<typeof setTimeout> }>();

const surface = document.getElementById("surface")!;
const list = document.getElementById("list")!;
const summary = document.getElementById("summary")!;
const grey = document.getElementById("grey")!;

// ---- Rows -------------------------------------------------------------

function renderRow(t: TileSnapshot): HTMLElement {
  const el = document.createElement("button");
  el.className = "row";
  el.setAttribute("aria-label", `Session ${t.index + 1}`);
  if (t.selected) el.classList.add("selected");

  const s = t.session;
  if (!s) {
    // Defensive only: every bound row should resolve to a session.
    el.classList.add("row-empty");
    el.textContent = "…";
    return el;
  }

  el.dataset["state"] = s.state;
  const spinning = s.state === "thinking" ? " spinning" : "";
  const glyph = s.state === "ended" ? GLYPHS["ended"] : GLYPHS[s.state];

  el.innerHTML = `
    <div class="glyph${spinning}">${glyph}</div>
    <div class="row-name">${escapeHtml(displayName(s))}</div>
    <div class="row-state">${escapeHtml(stateWord(s))}</div>`;

  const note = rowNotes.get(t.index);
  if (note) {
    const noteEl = document.createElement("div");
    noteEl.className = "row-note";
    noteEl.textContent = note.text;
    el.append(noteEl);
  }

  el.addEventListener("click", () => {
    void api.core
      .invoke("select_tile", { index: t.index })
      .then(() => api.core.invoke<string>("reveal_session", { index: t.index }))
      .then((text) => {
        if (!isRevealSuccess(text)) showRowNote(t.index, revealNote(text));
      });
  });
  return el;
}

function showRowNote(index: number, text: string): void {
  const existing = rowNotes.get(index);
  if (existing) clearTimeout(existing.timer);
  const timer = setTimeout(() => {
    rowNotes.delete(index);
    render();
  }, NOTE_MS);
  rowNotes.set(index, { text, timer });
  render();
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
