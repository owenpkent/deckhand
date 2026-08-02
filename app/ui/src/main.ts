// The Deckhand surface. It draws tiles and takes pointer input, and
// nothing else: no authority, no inference, no keyboard handlers at all
// (docs/ARCHITECTURE.md#the-surface, docs/ACCESSIBILITY.md).

import { applyTheme, dark } from "./theme.js";
import {
  BindableSession,
  SessionSnap,
  SessionState,
  Snapshot,
  tauri,
  TileSnapshot,
} from "./types.js";

const api = tauri();

let snapshot: Snapshot = { tiles: [], nowMs: Date.now() };
let pickerForTile: number | null = null;
let lastActivityAt = Date.now();

const IDLE_DIM_MS = 3 * 60 * 1000;

const strip = document.getElementById("strip")!;
const surface = document.getElementById("surface")!;
const picker = document.getElementById("picker")!;

// ---- Glyphs: drawn, never emoji (docs/UI_SPEC.md#state-rendering) ----

const GLYPHS: Record<string, string> = {
  idle: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="8"/></svg>`,
  thinking: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M12 4 a8 8 0 0 1 8 8"/></svg>`,
  needs_input: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 11V6a1.5 1.5 0 0 1 3 0v4V5a1.5 1.5 0 0 1 3 0v5V6.5a1.5 1.5 0 0 1 3 0V12v-2a1.5 1.5 0 0 1 3 0v5a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-2.7L4.6 14a1.6 1.6 0 0 1 2.6-1.8L8.5 14"/></svg>`,
  complete: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4 10-10"/></svg>`,
  error: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M6 6l12 12M18 6L6 18"/></svg>`,
  unknown: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M9 9a3 3 0 1 1 4.2 2.8c-.9.4-1.2 1-1.2 2.2"/><circle cx="12" cy="18" r="0.5" fill="currentColor"/></svg>`,
  ended: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 12h10"/></svg>`,
  plus: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 6v12M6 12h12"/></svg>`,
};

const STATE_WORDS: Record<SessionState, string> = {
  idle: "idle",
  thinking: "thinking",
  needs_input: "waiting on you",
  complete: "complete",
  error: "error",
  ended: "ended",
  unknown: "unknown",
};

// ---- Rendering ------------------------------------------------------

function fmtElapsed(fromMs: number, nowMs: number): string {
  const s = Math.max(0, Math.floor((nowMs - fromMs) / 1000));
  const m = Math.floor(s / 60);
  if (m >= 60) {
    return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
  }
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

function slot2Text(s: SessionSnap): string {
  if (s.state === "needs_input") {
    if (s.detailKind === "question") return "question";
    if (s.detailKind === "permission") return "permission";
    return "input needed";
  }
  if (s.state === "error" && s.error) return s.error.kind;
  if (s.openOps.length > 0) {
    const newest = s.openOps[s.openOps.length - 1];
    if (newest) return newest.tool;
  }
  if (s.detailTool) return s.detailTool;
  return STATE_WORDS[s.state];
}

function renderTile(t: TileSnapshot): HTMLElement {
  const el = document.createElement("button");
  el.className = "tile";
  el.setAttribute("aria-label", `Tile ${t.index + 1}`);
  if (t.selected) el.classList.add("selected");

  const s = t.session;
  if (!s) {
    el.classList.add("unbound");
    el.innerHTML = `
      <div class="glyph">${GLYPHS["plus"]}</div>
      <div class="slot2">bind</div>`;
    el.addEventListener("click", () => openPicker(t.index));
    return el;
  }

  el.dataset["state"] = s.state;
  const spinning = s.state === "thinking" ? " spinning" : "";
  const glyph = s.state === "ended" ? GLYPHS["ended"] : GLYPHS[s.state];

  // Slot 3: elapsed-in-operation while an operation is open, else
  // elapsed-in-state (docs/UI_SPEC.md#tile-anatomy).
  const oldestOp = s.openOps[0];
  const elapsedFrom = oldestOp ? oldestOp.openedAtMs : s.stateSinceMs;

  el.innerHTML = `
    <span class="badge-mode">${escapeHtml(s.permissionMode ?? "unknown")}</span>
    ${s.children > 0 ? `<span class="badge-children">&#215;${s.children}</span>` : ""}
    <div class="glyph${spinning}">${glyph}</div>
    <div class="slot1">${escapeHtml(s.label || s.id.slice(0, 8))}</div>
    <div class="slot2">${escapeHtml(slot2Text(s))}</div>
    <div class="slot3" data-from="${elapsedFrom}">${fmtElapsed(elapsedFrom, Date.now())}</div>`;

  el.addEventListener("click", () => {
    void api.core.invoke("select_tile", { index: t.index });
  });
  return el;
}

function render(): void {
  strip.replaceChildren(...snapshot.tiles.map(renderTile));
}

function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

// ---- Bind picker ----------------------------------------------------

async function openPicker(tileIndex: number): Promise<void> {
  pickerForTile = tileIndex;
  const sessions = await api.core.invoke<BindableSession[]>("bindable_sessions");
  picker.replaceChildren();

  const head = document.createElement("div");
  head.className = "picker-head";
  const label = document.createElement("span");
  label.textContent = `Bind tile ${tileIndex + 1}`;
  const cancel = document.createElement("button");
  cancel.className = "picker-btn";
  cancel.textContent = "Cancel";
  cancel.addEventListener("click", closePicker);
  head.append(label, cancel);

  const rows = document.createElement("div");
  rows.className = "picker-rows";
  if (sessions.length === 0) {
    const empty = document.createElement("div");
    empty.className = "picker-head";
    empty.textContent =
      "No sessions found. Scan again after a Claude Code session emits an event.";
    rows.append(empty);
  }
  for (const s of sessions) {
    const row = document.createElement("button");
    row.className = "picker-row";
    const name = document.createElement("span");
    name.textContent = s.label || s.id.slice(0, 8);
    const cwd = document.createElement("span");
    cwd.className = "row-cwd";
    cwd.textContent = s.cwd ?? "";
    const state = document.createElement("span");
    state.className = "row-state";
    state.textContent =
      s.boundTo !== null ? `${STATE_WORDS[s.state]} · tile ${s.boundTo + 1}` : STATE_WORDS[s.state];
    row.append(name, cwd, state);
    row.addEventListener("click", () => {
      void api.core
        .invoke("bind_tile", { index: tileIndex, sessionId: s.id })
        .then(closePicker);
    });
    rows.append(row);
  }

  picker.append(head, rows);
  picker.hidden = false;
}

function closePicker(): void {
  pickerForTile = null;
  picker.hidden = true;
}

// ---- Wiring ---------------------------------------------------------

function wake(): void {
  lastActivityAt = Date.now();
  surface.classList.remove("dimmed");
}

document.getElementById("refresh")!.addEventListener("click", () => {
  void api.core.invoke("refresh_sessions");
});

document.getElementById("quit")!.addEventListener("click", () => {
  void api.core.invoke("quit");
});

document.addEventListener("pointermove", wake);
document.addEventListener("pointerdown", wake);

// Elapsed readouts tick locally; a full re-render on every second would
// fight the pointer.
setInterval(() => {
  const now = Date.now();
  for (const el of Array.from(document.querySelectorAll<HTMLElement>(".slot3"))) {
    const from = Number(el.dataset["from"] ?? "0");
    if (from > 0) el.textContent = fmtElapsed(from, now);
  }
  if (now - lastActivityAt > IDLE_DIM_MS) {
    surface.classList.add("dimmed");
  }
}, 1000);

async function init(): Promise<void> {
  applyTheme(dark);
  await api.event.listen<Snapshot>("deckhand://snapshot", (e) => {
    snapshot = e.payload;
    wake(); // any state change wakes the idle dim
    render();
    if (pickerForTile !== null) {
      // Keep the picker fresh rather than stale under it.
      void openPicker(pickerForTile);
    }
  });
  snapshot = await api.core.invoke<Snapshot>("snapshot");
  render();
}

void init();
