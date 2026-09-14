// The Deckhand surface. It draws a list of sessions and takes pointer
// input, and nothing else: no authority, no inference, no keyboard
// handlers at all (docs/ARCHITECTURE.md#the-surface,
// docs/ACCESSIBILITY.md).
//
// One row per session, auto-bound by the daemon: there is no bind
// picker and no fixed row count. A click selects the row and raises its
// window in one motion (ADR-027); a raise that fails explains itself as
// a brief inline note on that row instead of opening anything.
//
// The header's gear button opens the settings panel in place of the
// session list (docs/DECISIONS.md#adr-033): always on top, start with
// Windows, reset position, hide unknown (moved here from the header,
// where ADR-030/031 had it), and a Hooks status row with a Repair
// action. The panel has no authority of its own either; it only shows
// what the daemon and the registry already hold and asks the daemon to
// change them.

import {
  displayName,
  escapeHtml,
  gearLabel,
  GLYPHS,
  hideUnknownText,
  hookStatusText,
  isRevealSuccess,
  onOffText,
  repairRowText,
  resetPositionText,
  revealNote,
  rowLabel,
  startWithWindowsText,
  STATE_WORDS,
  stateGlyph,
  stateWord,
  summaryCounts,
  unknownCount,
} from "./format.js";
import { presentSessionIds, staleNoteIds } from "./notes.js";
import { applyTheme, dark } from "./theme.js";
import { RepairOutcome, RepairResult, Snapshot, SettingsSnapshot, StartWithWindowsState, TileSnapshot, tauri } from "./types.js";

const api = tauri();

let snapshot: Snapshot = { tiles: [], nowMs: Date.now(), hideUnknown: false };
let lastActivityAt = Date.now();

const IDLE_DIM_MS = 3 * 60 * 1000;
const NOTE_MS = 4000;

// ---- Settings panel state ---------------------------------------------
//
// Pure navigation and in-flight-action state, never persisted here: the
// daemon is the source of truth for whether a setting is on, and for
// whether the panel itself is open (toggle_settings_panel's return
// value, not an optimistic local guess, is what panelOpen is set from).

let panelOpen = false;
let settings: SettingsSnapshot | null = null;
let repairRunning = false;
let repairOutcome: RepairOutcome | null = null;
let justResetPosition = false;
let resetNoteTimer: ReturnType<typeof setTimeout> | undefined;

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
const gear = document.getElementById("gear")!;

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

// ---- Header gear / settings panel --------------------------------------

// The gear's own visible text carries its pressed/open state (never
// colour alone, docs/ACCESSIBILITY.md): "Settings" closed, "Close
// settings" open.
function renderGear(): void {
  gear.setAttribute("aria-pressed", String(panelOpen));
  gear.textContent = gearLabel(panelOpen);
}

interface PanelRow {
  label: string;
  state: string;
  inactive?: boolean;
  onClick?: () => void;
}

function panelRows(): PanelRow[] {
  const s = settings;
  if (!s) return [];
  return [
    {
      label: "Always on top",
      state: onOffText(s.alwaysOnTop),
      onClick: () => {
        void api.core.invoke<boolean>("toggle_always_on_top").then((on) => {
          if (settings) settings.alwaysOnTop = on;
          render();
        });
      },
    },
    {
      label: "Start with Windows",
      state: startWithWindowsText(s.startWithWindows),
      onClick: () => {
        void api.core.invoke<StartWithWindowsState>("toggle_start_with_windows").then((state) => {
          if (settings) settings.startWithWindows = state;
          render();
        });
      },
    },
    {
      label: "Reset window position",
      state: resetPositionText(justResetPosition),
      onClick: () => {
        void api.core.invoke("reset_window_position").then(() => {
          justResetPosition = true;
          clearTimeout(resetNoteTimer);
          resetNoteTimer = setTimeout(() => {
            justResetPosition = false;
            render();
          }, NOTE_MS);
          render();
        });
      },
    },
    {
      label: "Hide unknown",
      state: hideUnknownText(snapshot.hideUnknown, unknownCount(snapshot.tiles)),
      onClick: () => {
        void api.core.invoke("toggle_hide_unknown");
      },
    },
    {
      label: "Hooks",
      state: hookStatusText(s.hookStatus),
    },
    {
      label: "Repair",
      state: repairRowText(s.installerAvailable, repairRunning, repairOutcome),
      // Not a native `disabled` button: docs/ACCESSIBILITY.md forbids a
      // control that clicking does nothing to explain, but this row's
      // reason is already shown as its permanent state text, with
      // nothing hidden behind the click, so a click while inactive is
      // an honest no-op rather than a silent dead one.
      inactive: !s.installerAvailable || repairRunning,
      onClick: () => {
        if (!settings || !settings.installerAvailable || repairRunning) return;
        repairRunning = true;
        repairOutcome = null;
        render();
        void api.core
          .invoke<RepairResult>("repair_hooks")
          .then((result) => {
            repairRunning = false;
            repairOutcome = result.outcome;
            if (settings) settings.hookStatus = result.hookStatus;
            render();
          })
          .catch(() => {
            repairRunning = false;
            repairOutcome = "failed_to_start";
            render();
          });
      },
    },
  ];
}

function renderPanelRow(row: PanelRow): HTMLElement {
  // The Hooks status row (no onClick) is read-only, like the header's
  // own summary counts, so it renders as a plain div rather than a
  // button that would imply a click does something.
  const el = document.createElement(row.onClick ? "button" : "div");
  el.className = "row panel-row";
  if (row.inactive) el.classList.add("panel-row-inactive");
  if (row.onClick) el.setAttribute("aria-disabled", String(!!row.inactive));
  el.innerHTML = `
    <div class="panel-row-label">${escapeHtml(row.label)}</div>
    <div class="panel-row-state">${escapeHtml(row.state)}</div>`;
  el.setAttribute("aria-label", `${row.label}, ${row.state}`);
  if (row.onClick) el.addEventListener("click", row.onClick);
  return el;
}

function renderPanel(): void {
  list.replaceChildren(...panelRows().map(renderPanelRow));
}

function render(): void {
  pruneRowNotes();
  renderSummary();
  renderGear();
  // The list container is reused for the panel rather than duplicated
  // (docs/DECISIONS.md#adr-033: "in-bar, not a separate window"), so its
  // accessible name has to say which one is actually showing.
  list.setAttribute("aria-label", panelOpen ? "Settings" : "Sessions");
  if (panelOpen) {
    renderPanel();
    return;
  }
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

gear.addEventListener("click", () => {
  void api.core.invoke<boolean>("toggle_settings_panel").then(async (open) => {
    panelOpen = open;
    if (open) {
      repairOutcome = null;
      repairRunning = false;
      settings = await api.core.invoke<SettingsSnapshot>("get_settings_snapshot");
    }
    render();
  });
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
