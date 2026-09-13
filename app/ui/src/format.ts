// Pure, DOM-free helpers shared by the surface. Nothing here touches the
// document, Tauri, or module-level mutable state, so these are safe to
// unit-test without a browser (docs/ARCHITECTURE.md#the-surface).

import { SessionSnap, SessionState } from "./types.js";

// ---- Glyphs: drawn, never emoji (docs/UI_SPEC.md#state-rendering) ----

export const GLYPHS: Record<string, string> = {
  idle: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="8"/></svg>`,
  thinking: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M12 4 a8 8 0 0 1 8 8"/></svg>`,
  needs_input: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 11V6a1.5 1.5 0 0 1 3 0v4V5a1.5 1.5 0 0 1 3 0v5V6.5a1.5 1.5 0 0 1 3 0V12v-2a1.5 1.5 0 0 1 3 0v5a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-2.7L4.6 14a1.6 1.6 0 0 1 2.6-1.8L8.5 14"/></svg>`,
  complete: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M5 13l4 4 10-10"/></svg>`,
  error: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M6 6l12 12M18 6L6 18"/></svg>`,
  unknown: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M9 9a3 3 0 1 1 4.2 2.8c-.9.4-1.2 1-1.2 2.2"/><circle cx="12" cy="18" r="0.5" fill="currentColor"/></svg>`,
  ended: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M7 12h10"/></svg>`,
  plus: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 6v12M6 12h12"/></svg>`,
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

// Shared by tiles, the panel identity line, and picker rows: prefer the
// human label, fall back to a short id fragment.
export function displayName(s: { id: string; label: string }): string {
  return s.label || s.id.slice(0, 8);
}

export function slot2Text(s: SessionSnap): string {
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
