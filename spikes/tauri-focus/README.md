# Spike: Tauri no-focus-steal window on Windows

Status: **passed**, on Windows 11 against Tauri 2 on 2026-08-02. Result
and hedges recorded as ADR-025 in `docs/DECISIONS.md`. Short version:
`alwaysOnTop` gives `WS_EX_TOPMOST` but not `WS_EX_NOACTIVATE`; one
`SetWindowLongPtrW` at setup adds it, and a click into the webview then
registers without the window ever taking the foreground.

The question, from `TODO.md` and ADR-009: can a Tauri v2 window on
Windows 11 be always-on-top and receive mouse clicks without ever taking
foreground focus from the window the user is working in? `alpha-osk`
proves the Win32 `WS_EX_NOACTIVATE | WS_EX_TOPMOST` approach works in
PySide6; this spike tests whether the same style bits hold when the
content area is a WebView2 webview inside a Tauri window.

## What it does

A single window (`ui/index.html`, no framework, no bundler) with one
button and an event log. On setup, the Rust side reads the window's
extended style, ORs in `WS_EX_NOACTIVATE` and `WS_EX_TOPMOST`, and
re-asserts topmost placement without activation. Every click and every
webview focus event is appended to `spike-log.jsonl` next to this file,
each line recording whether the foreground window at that moment was the
spike window itself.

Pass condition: clicks register in the log while `foreground_is_self`
stays `false` and the previously focused application keeps the
foreground.

## Run it

```powershell
cargo run --manifest-path spikes/tauri-focus/src-tauri/Cargo.toml
```

`scripts/focus-test.ps1` in this directory automates the check: it
starts Notepad, records the foreground window, clicks the spike button
with a synthetic mouse event, and verifies the foreground window did not
change while the click was received.
