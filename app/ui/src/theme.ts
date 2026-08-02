// Theme tokens. Nothing outside this module hardcodes a hex value
// (docs/UI_SPEC.md#theming). The six state colours are semantic tokens
// frozen in meaning by ADR-008; a theme may shift luminance, never the
// hue mapping.

export interface Theme {
  bg: string;
  tile: string;
  tileBorder: string;
  text: string;
  subtext: string;
  // The six frozen state colours plus unknown's grey.
  idle: string;
  thinking: string;
  needsInput: string;
  complete: string;
  error: string;
  unknown: string;
}

export const dark: Theme = {
  bg: "#14161a",
  tile: "#1e2128",
  tileBorder: "#3a3f47",
  text: "#f2f4f6",
  subtext: "#b8c2cc",
  idle: "#e8eaed",
  thinking: "#4c8dff",
  needsInput: "#f5a623",
  complete: "#34c759",
  error: "#ff453a",
  unknown: "#8e8e93",
};

export function applyTheme(t: Theme): void {
  const root = document.documentElement.style;
  root.setProperty("--bg", t.bg);
  root.setProperty("--tile", t.tile);
  root.setProperty("--tile-border", t.tileBorder);
  root.setProperty("--text", t.text);
  root.setProperty("--subtext", t.subtext);
  root.setProperty("--c-idle", t.idle);
  root.setProperty("--c-thinking", t.thinking);
  root.setProperty("--c-needs-input", t.needsInput);
  root.setProperty("--c-complete", t.complete);
  root.setProperty("--c-error", t.error);
  root.setProperty("--c-unknown", t.unknown);
}
