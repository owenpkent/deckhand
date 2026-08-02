// The Deckhand surface. It draws controls and takes pointer input, and
// nothing else: no authority, no inference, no keyboard handlers at all
// (docs/ARCHITECTURE.md#the-surface, docs/ACCESSIBILITY.md).
//
// Every control ships visible. A control that cannot act is dimmed, and
// clicking it is never a no-op: the reason lands in the detail panel in
// words, expanding the panel first if it was collapsed
// (docs/UI_SPEC.md#command-keys).

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
let panelExpanded = false;
let panelMessage = "";
let previousTile: number | null = null;
let lastActivityAt = Date.now();

const IDLE_DIM_MS = 3 * 60 * 1000;

const strip = document.getElementById("strip")!;
const surface = document.getElementById("surface")!;
const panel = document.getElementById("panel")!;
const picker = document.getElementById("picker")!;
const keys = document.getElementById("keys")!;
const stick = document.getElementById("stick")!;
const dial = document.getElementById("dial")!;
const talkcol = document.getElementById("talkcol")!;

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

// ---- Small helpers --------------------------------------------------

function fmtElapsed(fromMs: number, nowMs: number): string {
  const s = Math.max(0, Math.floor((nowMs - fromMs) / 1000));
  const m = Math.floor(s / 60);
  if (m >= 60) {
    return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}`;
  }
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function selectedTile(): TileSnapshot | null {
  return snapshot.tiles.find((t) => t.selected) ?? null;
}

function selectedSession(): SessionSnap | null {
  return selectedTile()?.session ?? null;
}

// ---- Reveal-the-reason plumbing -------------------------------------

async function setPanel(expanded: boolean): Promise<void> {
  panelExpanded = expanded;
  await api.core.invoke("set_panel_expanded", { expanded });
  renderPanel();
}

function showMessage(text: string): void {
  panelMessage = text;
  if (!panelExpanded) {
    void setPanel(true);
  } else {
    renderPanel();
  }
}

// ---- Tiles ----------------------------------------------------------

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
    const prev = selectedTile();
    if (prev && prev.index !== t.index) previousTile = prev.index;
    void api.core.invoke("select_tile", { index: t.index });
  });
  return el;
}

// ---- Command keys ---------------------------------------------------

interface KeyDef {
  cls: string;
  glyph: string;
  label: string;
  // Returns null when the key can act, else the reason it cannot.
  blocked(): string | null;
  act(): void;
}

const KEY_DEFS: KeyDef[] = [
  {
    cls: "key-approve",
    glyph: "✔",
    label: "Approve",
    blocked: () => {
      const s = selectedSession();
      if (!s) return "No tile is selected.";
      if (s.state !== "needs_input") return "No pending permission request. Approve and Deny arrive with the Phase 2 gate.";
      if (s.detailKind === "question") return "This is a question, not a permission. The options are in this panel.";
      if (s.detailKind === null) return "The runtime did not say which sort of prompt this is, so Approve stays dark rather than guess.";
      return "The Phase 2 permission gate is not built yet; nothing in Deckhand can approve a tool call today.";
    },
    act: () => {},
  },
  {
    cls: "key-deny",
    glyph: "✘",
    label: "Deny",
    blocked: () => {
      const s = selectedSession();
      if (!s) return "No tile is selected.";
      if (s.state !== "needs_input") return "No pending permission request. Approve and Deny arrive with the Phase 2 gate.";
      if (s.detailKind === "question") return "This is a question, not a permission. The options are in this panel.";
      return "The Phase 2 permission gate is not built yet; nothing in Deckhand can deny a tool call today.";
    },
    act: () => {},
  },
  {
    cls: "key-answer",
    glyph: "?",
    label: "Answer",
    blocked: () => {
      const s = selectedSession();
      if (!s) return "No tile is selected.";
      if (s.state !== "needs_input" || s.detailKind !== "question") return "The selected session has no question waiting.";
      return "No answer channel is proven on any runtime (ADR-013), so the options below are shown but cannot be clicked into the session yet.";
    },
    act: () => {},
  },
  {
    cls: "key-interrupt",
    glyph: "■",
    label: "Interrupt",
    blocked: () => {
      const s = selectedSession();
      if (!s) return "No tile is selected.";
      if (s.state !== "thinking") return "The selected session is not running a turn.";
      return "No interrupt channel is proven from outside a session (ADR-020). The opt-in keystroke fallback is not implemented.";
    },
    act: () => {},
  },
  {
    cls: "key-continue",
    glyph: "▶",
    label: "Continue",
    blocked: () =>
      "No send channel exists in attached mode: the documented channels deliver at a turn boundary, never into an idle session (ADR-020).",
    act: () => {},
  },
  {
    cls: "key-reveal",
    glyph: "◎",
    label: "Reveal",
    blocked: () => {
      const t = selectedTile();
      if (!t || !t.session) return "No session is selected to reveal.";
      return null;
    },
    act: () => {
      const t = selectedTile();
      if (!t) return;
      void api.core
        .invoke<string>("reveal_session", { index: t.index })
        .then(showMessage);
    },
  },
];

function renderKeys(): void {
  keys.replaceChildren(
    ...KEY_DEFS.map((def) => {
      const el = document.createElement("button");
      const reason = def.blocked();
      el.className = `ctl ${def.cls}${reason ? " off" : ""}`;
      el.innerHTML = `<span class="k-glyph">${def.glyph}</span><span class="k-label">${def.label}</span>`;
      el.addEventListener("click", () => {
        const r = def.blocked();
        if (r) showMessage(r);
        else def.act();
      });
      return el;
    })
  );
}

// ---- Stick, dial, talk ----------------------------------------------

function renderStick(): void {
  const defs = [
    {
      glyph: "▲",
      label: "Up",
      act: () => scrollPanel(-64),
    },
    {
      glyph: "▼",
      label: "Down",
      act: () => scrollPanel(64),
    },
    {
      glyph: "◀",
      label: "Back",
      act: () => {
        if (previousTile !== null) {
          void api.core.invoke("select_tile", { index: previousTile });
        } else {
          showMessage("No previously selected tile to return to.");
        }
      },
    },
    {
      glyph: "▶",
      label: "Panel",
      act: () => void setPanel(!panelExpanded),
    },
  ];
  stick.replaceChildren(
    ...defs.map((d) => {
      const el = document.createElement("button");
      el.className = "ctl";
      el.style.width = "44px";
      el.style.height = "44px";
      el.innerHTML = `<span class="k-glyph">${d.glyph}</span><span class="k-label">${d.label}</span>`;
      el.addEventListener("click", d.act);
      return el;
    })
  );
}

function scrollPanel(step: number): void {
  if (!panelExpanded) {
    void setPanel(true);
    return;
  }
  const body = panel.querySelector(".panel-main");
  if (body) body.scrollBy({ top: step });
}

const DIAL_REASON =
  "The dial is a readout in attached mode: no observed interface sets model, effort, or permission mode on a running session. The commit target arrives with hosted mode.";

function renderDial(): void {
  dial.replaceChildren();
  const commit = document.createElement("button");
  commit.className = "ctl off dial-commit";
  commit.innerHTML = `<span class="k-glyph">●</span>`;
  commit.setAttribute("aria-label", "Dial commit");
  commit.addEventListener("click", () => showMessage(DIAL_REASON));
  const steppers = document.createElement("div");
  steppers.className = "dial-steppers";
  for (const g of ["−", "+"]) {
    const b = document.createElement("button");
    b.className = "ctl off";
    b.innerHTML = `<span class="k-glyph">${g}</span>`;
    b.addEventListener("click", () => showMessage(DIAL_REASON));
    steppers.append(b);
  }
  dial.append(commit, steppers);
}

function renderTalk(): void {
  talkcol.replaceChildren();
  const talk = document.createElement("button");
  talk.className = "ctl off";
  talk.innerHTML = `<span class="k-glyph">⊙</span><span class="k-label">Talk</span>`;
  talk.addEventListener("click", () =>
    showMessage("Talk arrives in Phase 5, delegated to MacroVox. Deckhand itself never touches the microphone.")
  );
  const send = document.createElement("button");
  send.className = "ctl off";
  send.innerHTML = `<span class="k-glyph">➤</span><span class="k-label">Send</span>`;
  send.addEventListener("click", () =>
    showMessage("No send channel exists in attached mode (ADR-020), and there is nothing composed to send.")
  );
  talkcol.append(talk, send);
}

// ---- Detail panel ---------------------------------------------------

function renderPanel(): void {
  panel.hidden = !panelExpanded;
  if (!panelExpanded) return;

  panel.replaceChildren();
  const main = document.createElement("div");
  main.className = "panel-main";

  const t = selectedTile();
  const s = t?.session ?? null;

  const identity = document.createElement("div");
  identity.className = "panel-identity";
  identity.textContent = t
    ? s
      ? `tile ${t.index + 1} · ${s.label || s.id.slice(0, 8)} · ${s.permissionMode ?? "unknown"} · attached`
      : `tile ${t.index + 1} · unbound`
    : "no tile selected";

  main.append(identity);

  if (s) {
    const state = document.createElement("div");
    state.className = "panel-state";
    state.style.color = `var(--c-${s.state.replace("_", "-")}, var(--text))`;
    state.textContent = STATE_WORDS[s.state];
    main.append(state);

    if (s.question) {
      const q = document.createElement("div");
      q.className = "panel-question";
      q.textContent = s.question;
      const targets = document.createElement("div");
      targets.className = "answer-targets";
      for (const opt of s.options) {
        const b = document.createElement("button");
        b.className = "ctl off";
        b.textContent = opt;
        b.addEventListener("click", () =>
          showMessage("This session has no answer channel; answer in its own window. Reveal raises it.")
        );
        targets.append(b);
      }
      main.append(q, targets);
    }

    const item = document.createElement("div");
    item.className = "panel-item";
    const oldest = s.openOps[0];
    if (oldest) {
      item.textContent = `${oldest.tool} · running ${fmtElapsed(oldest.openedAtMs, Date.now())}`;
    } else if (s.error) {
      item.textContent = `${s.error.kind}${s.error.message ? `: ${s.error.message}` : ""}`;
    } else if (s.detailTool) {
      item.textContent = `last: ${s.detailTool}`;
    }
    if (item.textContent) main.append(item);
  }

  const message = document.createElement("div");
  message.className = "panel-message";
  message.textContent = panelMessage;
  main.append(message);

  const actions = document.createElement("div");
  actions.className = "panel-actions";

  const mkAction = (label: string, off: boolean, onClick: () => void) => {
    const b = document.createElement("button");
    b.className = `ctl${off ? " off" : ""}`;
    b.textContent = label;
    b.addEventListener("click", onClick);
    actions.append(b);
  };

  mkAction("Reveal", !s, () => {
    if (!t || !s) {
      showMessage("No session is selected to reveal.");
      return;
    }
    void api.core.invoke<string>("reveal_session", { index: t.index }).then(showMessage);
  });
  mkAction("Unbind", !s, () => {
    if (!t || !s) {
      showMessage("This tile has no binding to remove.");
      return;
    }
    void api.core.invoke("unbind_tile", { index: t.index });
  });
  mkAction("Scan", false, () => {
    showMessage("Scanning for live sessions…");
    void api.core.invoke("refresh_sessions").then(() => showMessage("Scan finished."));
  });

  panel.append(main, actions);
}

// ---- Render root ----------------------------------------------------

function render(): void {
  strip.replaceChildren(...snapshot.tiles.map(renderTile));
  renderKeys();
  renderPanel();
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
  const scan = document.createElement("button");
  scan.className = "picker-btn";
  scan.textContent = "Scan again";
  scan.addEventListener("click", () => {
    void api.core.invoke("refresh_sessions").then(() => openPicker(tileIndex));
  });
  const cancel = document.createElement("button");
  cancel.className = "picker-btn";
  cancel.textContent = "Cancel";
  cancel.addEventListener("click", closePicker);
  const headBtns = document.createElement("div");
  headBtns.style.display = "flex";
  headBtns.style.gap = "8px";
  headBtns.append(scan, cancel);
  head.append(label, headBtns);

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

document.getElementById("move")!.addEventListener("click", () => {
  void api.core.invoke("cycle_position");
});

document.getElementById("quit")!.addEventListener("click", () => {
  void api.core.invoke("quit");
});

document.addEventListener("pointermove", wake);
document.addEventListener("pointerdown", wake);

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
  renderStick();
  renderDial();
  renderTalk();
  await api.event.listen<Snapshot>("deckhand://snapshot", (e) => {
    snapshot = e.payload;
    wake();
    render();
    if (pickerForTile !== null) {
      void openPicker(pickerForTile);
    }
  });
  snapshot = await api.core.invoke<Snapshot>("snapshot");
  render();
}

void init();
