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
// session list (docs/DECISIONS.md#adr-033, restyled by adr-034): always
// on top, start with Windows, and reset position under "Window"; hide
// unknown under "List"; and a combined Hooks status and Repair row
// under "Claude Code". The panel has no authority of its own either; it
// only shows what the daemon and the registry already hold and asks the
// daemon to change them.

import {
  boolChecked,
  countPillLabel,
  displayName,
  escapeHtml,
  gearIconKind,
  GLYPHS,
  hideUnknownText,
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
  stateGlyph,
  stateWord,
  summaryCounts,
  unknownCount,
} from "./format.js";
import { BACK_ICON, GEAR_ICON, RESET_ICON } from "./icons.js";
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
      el.setAttribute("aria-label", countPillLabel(state, n));
      el.innerHTML = `<span class="glyph">${GLYPHS[state]}</span>${n}`;
      return el;
    }),
  );
}

// ---- Header gear / settings panel --------------------------------------

// The gear's own icon carries its pressed/open state (never colour
// alone, docs/ACCESSIBILITY.md): the cog closed, an arrow back to the
// session list open. aria-pressed carries the same fact for anyone not
// reading the shape; aria-label stays the fixed "Settings" set in
// index.html either way.
function renderGear(): void {
  gear.setAttribute("aria-pressed", String(panelOpen));
  gear.innerHTML = gearIconKind(panelOpen) === "back" ? BACK_ICON : GEAR_ICON;
}

// ---- Settings panel rows (docs/DECISIONS.md#adr-034) -------------------
//
// Three kinds of row. A switch row is the whole row acting as a
// role="switch" control (never a button nested inside a button): the
// track-and-thumb graphic is decorative, aria-hidden, and the On/Off
// word next to it is the real, always-visible state text. An action row
// is a plain button that does something once, not a toggle (Reset
// position). The Hooks row is the one row with no click of its own
// (like the header's own summary, it is a plain div), but it holds a
// real nested <button> for Repair, which a div is free to contain.

interface SwitchRow {
  kind: "switch";
  key: string;
  label: string;
  checked: boolean;
  word: string;
  onClick: () => void;
}

interface ActionRow {
  kind: "action";
  key: string;
  label: string;
  secondary: string;
  icon: string;
  onClick: () => void;
}

interface HooksRow {
  kind: "hooks";
  key: string;
  label: string;
  pillText: string;
  pillClass: string;
  secondary: string;
  repairInactive: boolean;
  onRepair: () => void;
}

type PanelRow = SwitchRow | ActionRow | HooksRow;

interface PanelSection {
  title: string;
  rows: PanelRow[];
}

function panelSections(): PanelSection[] {
  const s = settings;
  if (!s) return [];
  return [
    {
      title: "Window",
      rows: [
        {
          kind: "switch",
          key: "always-on-top",
          label: "Always on top",
          checked: s.alwaysOnTop,
          word: onOffText(s.alwaysOnTop),
          onClick: () => {
            void api.core.invoke<boolean>("toggle_always_on_top").then((on) => {
              if (settings) settings.alwaysOnTop = on;
              render();
            });
          },
        },
        {
          kind: "switch",
          key: "start-with-windows",
          label: "Start with Windows",
          checked: startWithWindowsChecked(s.startWithWindows) === "true",
          word: startWithWindowsText(s.startWithWindows),
          onClick: () => {
            void api.core.invoke<StartWithWindowsState>("toggle_start_with_windows").then((state) => {
              if (settings) settings.startWithWindows = state;
              render();
            });
          },
        },
        {
          kind: "action",
          key: "reset-position",
          label: "Reset position",
          secondary: resetPositionText(justResetPosition),
          icon: RESET_ICON,
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
      ],
    },
    {
      title: "List",
      rows: [
        {
          kind: "switch",
          key: "hide-unknown",
          label: "Hide unknown",
          checked: snapshot.hideUnknown,
          word: hideUnknownText(snapshot.hideUnknown, unknownCount(snapshot.tiles)),
          onClick: () => {
            void api.core.invoke("toggle_hide_unknown");
          },
        },
      ],
    },
    {
      title: "Claude Code",
      rows: [
        {
          kind: "hooks",
          key: "hooks",
          label: "Hooks",
          pillText: hookStatusText(s.hookStatus),
          pillClass: hookStatusPillClass(s.hookStatus),
          secondary: repairSecondaryText(s.installerAvailable, repairRunning, repairOutcome),
          repairInactive: repairButtonInactive(s.installerAvailable, repairRunning),
          onRepair: () => {
            if (!settings || repairButtonInactive(settings.installerAvailable, repairRunning)) return;
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
      ],
    },
  ];
}

function renderSwitchRow(row: SwitchRow): HTMLElement {
  const el = document.createElement("button");
  el.className = "row panel-row panel-row-switch";
  el.setAttribute("role", "switch");
  el.setAttribute("aria-checked", boolChecked(row.checked));
  el.innerHTML = `
    <div class="panel-row-main">
      <div class="panel-row-label">${escapeHtml(row.label)}</div>
    </div>
    <div class="panel-row-control">
      <span class="switch" aria-hidden="true"><span class="switch-thumb"></span></span>
      <span class="switch-word">${escapeHtml(row.word)}</span>
    </div>`;
  el.setAttribute("aria-label", `${row.label}, ${row.word}`);
  el.addEventListener("click", row.onClick);
  return el;
}

function renderActionRow(row: ActionRow): HTMLElement {
  const el = document.createElement("button");
  el.className = "row panel-row panel-row-action";
  el.innerHTML = `
    <div class="panel-row-main">
      <div class="panel-row-label">${escapeHtml(row.label)}</div>
      <div class="panel-row-secondary">${escapeHtml(row.secondary)}</div>
    </div>
    <div class="panel-row-control glyph" aria-hidden="true">${row.icon}</div>`;
  el.setAttribute("aria-label", `${row.label}, ${row.secondary}`);
  el.addEventListener("click", row.onClick);
  return el;
}

function renderHooksRow(row: HooksRow): HTMLElement {
  const el = document.createElement("div");
  el.className = "row panel-row panel-row-hooks";
  el.innerHTML = `
    <div class="panel-row-main">
      <div class="panel-row-label-line">
        <span class="panel-row-label">${escapeHtml(row.label)}</span>
        <span class="pill ${row.pillClass}">${escapeHtml(row.pillText)}</span>
      </div>
      <div class="panel-row-secondary">${escapeHtml(row.secondary)}</div>
    </div>
    <button type="button" class="repair-btn"${row.repairInactive ? ' data-inactive="true"' : ""} aria-label="Repair hooks">Repair</button>`;
  el.setAttribute(
    "aria-label",
    `${row.label}, ${row.pillText}${row.secondary ? `, ${row.secondary}` : ""}`,
  );
  // Not a native `disabled` button: docs/ACCESSIBILITY.md forbids a
  // control that clicking does nothing to explain, but this row's
  // reason is already shown as its permanent secondary text, so a click
  // while inactive is an honest no-op rather than a silent dead one.
  // onRepair itself re-checks repairInactive before doing anything.
  el.querySelector(".repair-btn")!.addEventListener("click", row.onRepair);
  return el;
}

function renderPanelRow(row: PanelRow): HTMLElement {
  switch (row.kind) {
    case "switch":
      return renderSwitchRow(row);
    case "action":
      return renderActionRow(row);
    case "hooks":
      return renderHooksRow(row);
  }
}

function renderPanel(): void {
  list.replaceChildren(
    ...panelSections().map((section) => {
      const wrap = document.createElement("div");
      wrap.className = "settings-section";
      const title = document.createElement("div");
      title.className = "settings-section-title";
      title.textContent = section.title;
      const card = document.createElement("div");
      card.className = "settings-card";
      card.append(...section.rows.map(renderPanelRow));
      wrap.append(title, card);
      return wrap;
    }),
  );
}

function render(): void {
  pruneRowNotes();
  renderSummary();
  renderGear();
  // The list container is reused for the panel rather than duplicated
  // (docs/DECISIONS.md#adr-033: "in-bar, not a separate window"), so its
  // accessible name has to say which one is actually showing.
  list.setAttribute("aria-label", panelOpen ? "Settings" : "Sessions");
  // Panel-only spacing (docs/DECISIONS.md#adr-034); the session list
  // keeps its own flush layout so its height stays exactly
  // rows * ROW_H_LOGICAL, matching window.rs.
  list.classList.toggle("panel", panelOpen);
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
