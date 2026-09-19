# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a
Changelog](https://keepachangelog.com/en/1.1.0/), and dates are ISO 8601
(`YYYY-MM-DD`). There are no releases yet, so everything so far lives
under `[Unreleased]`. Once there is something to version, releases here
will follow [Semantic Versioning](https://semver.org/); until then, no
version number is invented and no past release is backfilled.

## [Unreleased]

### Added

- **Anchor noted as a candidate hosted-mode engine.** `IDEAS.md` records
  what the sibling Anchor project already has (Agent SDK session
  ownership, an approval broker that fails to "the tool does not run", a
  hash-chained audit log, a three-method channel protocol) and what it
  does not do (observe attached Claude Code sessions). `ROADMAP.md` Phase 4
  points at the entry. No decision, no code, and no new dependency.
- **`docs/UPSTREAM_ASKS.md`, one place for what Deckhand needs from the
  runtimes it observes.** Each entry pairs a need with the workaround
  standing in for it and the file that carries the cost, so an issue can
  be filed from evidence rather than rewritten from memory. Checking the
  contribution paths first ruled out a pull request to either vendor:
  `anthropics/claude-code` ships no CLI source and is "All rights
  reserved", and `openai/codex` states it does not accept external code
  contributions, so issues are the only channel to both. Two findings
  came out of the check and are recorded with a 2.1.273 stamp: the CLI
  already writes `entrypoint`, `procStart`, `nameSource`, and a
  `messagingSocketPath` with `peerFeatures: ["notify_idle",
  "artifact_yield"]` to `~/.claude/sessions/<pid>.json`, and
  `claude agents --json` drops all four, which makes the smallest ask on
  the list also the most valuable one. Nothing has been filed yet and no
  adapter capability changes on the strength of this file.

  Checking the asks against the current release before writing them down
  also retired one of them. The waiting-on-human event Deckhand wanted
  already shipped: `anthropics/claude-code#13024` closed as completed on
  2026-08-17, and a `PermissionRequest` hook fires immediately, while
  `Notification`/`permission_prompt` waits roughly six seconds of
  inactivity, which explains why `Notification` has never been observed
  here. Deckhand registers twelve events and `PermissionRequest` is not
  one of them, so that is now Phase 1 work in `TODO.md` rather than an
  ask. Recorded in section 4.3 with the correction left visible.

### Changed

- **The hardware macropad is no longer named in the living
  documentation.** README, IDEAS, SECURITY, CLAUDE.md, ACCESSIBILITY,
  CONTROL_MAPPING, EXECUTIVE_SUMMARY, and WHITEPAPER now describe the
  inspiration generically ("a limited-run hardware macropad", "the
  original device") instead of by product name. README's tagline becomes
  "A software control surface for Claude Code sessions" and
  `docs/CONTROL_MAPPING.md` becomes "Control mapping: hardware to
  Deckhand". The related-projects table row and the acknowledgement line
  crediting the device's makers are removed, and the executive summary no
  longer claims the inspiration "deserves to be credited plainly", which
  had become self-contradictory once the name was gone. The facts are
  unchanged: Deckhand is still a deliberate software reinterpretation of
  a hardware design, and the documentation still says so.

  `docs/DECISIONS.md` and this file keep the original wording on purpose.
  ADRs are append-only and ADR-001's title carries its `#adr-001` anchor,
  so rewriting it would break inbound links and the repository's own
  rule; a changelog records what was true when it was written. The
  non-affiliation disclaimers in README, EXECUTIVE_SUMMARY, and
  WHITEPAPER are also unchanged, since they are legal statements rather
  than credit.

### Fixed

- **Hidden superseded rows and consistent labels**
  ([ADR-038](docs/DECISIONS.md#adr-038)). The VS Code extension was
  observed keeping an older session's `claude.exe` alive after a newer
  session started in the same window, so the board showed what looked
  like duplicate rows, spelled two different ways: a hook-first row took
  its label from its directory, a scan-first row took the scan's own
  `name`. An idle, complete, or unknown session is now left off the list
  while a newer bound session shares its VS Code window, its parent
  process, and its folder; the older row keeps its binding and reappears
  the moment it needs the owner or the newer one ends. A row's label now
  follows one rule regardless of which channel saw it first: the scan's
  `name` when there is one, the directory name otherwise, tracked by a
  `derived` flag persisted in `bindings.json` rather than guessed by
  comparing strings. `app/src-tauri/src/supersede.rs` holds the rule;
  `docs/ARCHITECTURE.md`, `docs/CLAUDE_CODE_ADAPTER.md`,
  `docs/UI_SPEC.md`, `docs/CONTROL_MAPPING.md`, and `TODO.md` are updated
  to match.

### Added

- **A technical white paper** ([docs/WHITEPAPER.md](docs/WHITEPAPER.md)).
  One document covering the problem, the surface, the architecture, how
  the three observation channels are reconciled, the Windows integration,
  the security model, the accessibility rules, the evidence behind each
  integration claim, and the limitations. Derived, never authoritative.
  `scripts/build-whitepaper.ps1` typesets it to a PDF with pandoc and
  tectonic, into `target\whitepaper\`.

- **Process liveness: a handle per session, exit ends the row**
  ([ADR-035](docs/DECISIONS.md#adr-035)). When a scan reports a pid for a
  session, the daemon opens an `OpenProcess(SYNCHRONIZE)` handle on it and
  keeps it for the session's life, polled with a zero-timeout wait on the
  existing two-second tick. Exit without a `SessionEnd` now moves the
  session straight to `ended` and off the list, exactly as `SessionEnd`
  would; only a `SessionStart` revives it. A handle is opened only from a
  scan sighting, never from a pid restored from disk, since a restored pid
  may already name another process.

- **Single instance and a crash watchdog**
  ([ADR-037](docs/DECISIONS.md#adr-037)). Startup now claims a named
  kernel mutex, `Local\Deckhand.Instance`; a second launch raises the
  first copy's window without activating it and exits 0 rather than
  opening a duplicate. Every launch also spawns a detached watchdog, the
  same `deckhand.exe` in a headless `--watchdog <pid>` mode, that waits
  for the app to exit and relaunches it on any exit code other than 0,
  rate-limited by an append-only local ledger,
  `%LOCALAPPDATA%\deckhand\watchdog.log` (three restarts in ten minutes
  and the watchdog gives up rather than loop forever). Start with
  Windows is unchanged. Two `deckhand.exe` processes now run while the
  board is up.

### Changed

- **The scan colours a never-heard session, `T_unknown` narrows, and list
  membership follows the handle** ([ADR-035](docs/DECISIONS.md#adr-035)).
  `claude agents --json`'s `status` key, absent on 2.1.220, is present on
  the installed 2.1.270: `busy` and `shell` map to `thinking`, `waiting` to
  `needs_input`, `idle` to `idle`, applied only to a session no hook has
  coloured yet and never producing `complete`. `T_unknown`
  (900 s, [ADR-016](docs/DECISIONS.md#adr-016)) now narrows for a session
  with a live handle: `idle`, `complete`, `error`, and `needs_input` hold
  for as long as the process lives, and only a `thinking` session the scan
  does not confirm `busy`, `shell`, or `waiting` still times out to
  `unknown`. A session with no handle keeps the old rule, except a
  successful scan sighting now also counts as an event of any kind. A
  bound session now leaves the list on `SessionEnd`, on the handle
  reporting the process gone, or, only for a session with no handle, on
  the existing scan-plus-sixty-seconds prune. `docs/ARCHITECTURE.md`,
  `docs/CLAUDE_CODE_ADAPTER.md`, `TODO.md`, and `CLAUDE.md` are updated to
  match, and `app/` implements the change.

- **The scan breaks ties with hooks after two consecutive contradicting
  scans** ([ADR-036](docs/DECISIONS.md#adr-036)). Once a hook has
  coloured a session, the scan still does not recolour it on a single
  disagreement, but after two consecutive scans contradict the hook-set
  colour with no hook event between them (about thirty seconds at the
  fifteen-second rescan), a `thinking` session the scan reports `idle`
  moves to `idle`, clearing its open operations and child ledger, and an
  `idle`, `complete`, or `error` session the scan reports `busy` or
  `shell` moves to `thinking`. Any hook event resets the count; the scan
  still never produces `complete`, still never touches `needs_input`, and
  `waiting` never triggers the tie-break. `docs/ARCHITECTURE.md` and
  `docs/CLAUDE_CODE_ADAPTER.md` are updated to match, and `app/`
  implements the change.

### Removed

- **The transcript JSONL fallback, retired before being built**
  ([ADR-036](docs/DECISIONS.md#adr-036)). Deckhand does not read session
  transcripts: hooks and `claude agents --json` are the only observation
  channels, and the `transcript_path` field hooks carry stays unused. The
  scan's `status`, observed on 2.1.270, already answers what the fallback
  would have, from a documented command rather than an undocumented file
  with no stability promise. `docs/ARCHITECTURE.md`,
  `docs/CLAUDE_CODE_ADAPTER.md`, `docs/EXECUTIVE_SUMMARY.md`, and
  `TODO.md` are updated to match.

### Changed

- **A settings panel, opened by a new gear button added beside the
  header's grey toggle** ([ADR-033](docs/DECISIONS.md#adr-033)). The
  panel replaces the session list in place (not a second window),
  resizing the same window through the existing resize path, and holds
  five rows: Always on top (on by default, persisted, reapplies
  `WS_EX_NOACTIVATE` after any topmost toggle and restores the taskbar
  icon when off), Start with Windows (an `HKCU\...\Run` registry value
  named `Deckhand`, read fresh on every toggle, tri-state so a copy
  already pointed at a different exe reads "On (other copy)" instead of
  silently turning off), Reset window position (moves the window to its
  default placement and persists it), and a Hooks status row
  ("Installed," "Outdated," "Missing," or "Unreadable," from a pure
  parse of `~/.claude/settings.json`) with a Repair action that reruns
  `scripts/install-hooks.ps1` off the UI thread, bounded at 20 seconds.
  The header's grey toggle stays exactly where it was; it does not move
  into the panel. New Tauri commands (`toggle_settings_panel`,
  `get_settings_snapshot`, `toggle_always_on_top`,
  `toggle_start_with_windows`, `reset_window_position`, `repair_hooks`)
  take no argument from the webview, matching `toggle_hide_unknown`'s
  existing shape. The Repair row is styled inactive rather than
  natively disabled when no installer checkout is found nearby, since
  its reason is already permanent, visible state text
  (docs/ACCESSIBILITY.md forbids a disabled control that clicking
  explains nothing).

### Changed

- **Visual refresh of the header, settings panel, and session rows**
  ([ADR-034](docs/DECISIONS.md#adr-034)). The gear and Quit are now
  inline svg icon buttons (`app/ui/src/icons.ts`) instead of text; the
  gear's open state is a shape swap, cog to back arrow, plus a raised
  background, not only a colour change. In the panel, Always on top
  and Start with Windows render as a toggle switch (`role="switch"`,
  `aria-checked`) beside their existing On/Off word, its state word to
  the left, right-aligned against the switch's own left edge, so the
  switch, the Reset row's icon, and the Repair button all end flush
  against the same right edge. The header's own grey toggle gets the
  same switch, and its wording shortens to "Hide grey" / "N hidden"
  (was "Hide unknown" / "Show N unknown"). Reset window position is
  now a two-line action row labelled "Reset position," and the Hooks
  and Repair rows combine into one: a tinted status pill plus a real
  Repair button, with Repair's own result as a secondary line. The
  panel's four rows now sit in two titled sections, Window and Claude
  Code, instead of one flat list. Session rows and panel cards both
  gain rounded corners, a left accent bar in the row's own state
  colour, a softer background tint (idle 5%, thinking and complete 8%,
  needs input and error 14%, down from 6/14/22), and a shared 8 px
  horizontal inset from the window's left and right edges; none of
  ADR-008's six colours or their meanings change. The window now sizes
  the open panel off a fixed formula (`panel_content_height_logical`
  and `panel_window_height` in `window.rs`) instead of a flat row
  count times `ROW_H_LOGICAL`, fixing a native scrollbar that used to
  appear because the two had drifted apart.

### Fixed

- **`ENDED` now absorbs every straggler, not just a clear or a resume.**
  `Session::apply_hook` (`state.rs`) had no guard for a session already in
  `ENDED`: a `Stop` or a `PostToolUseFailure` delivered late, or racing
  `SessionEnd` itself, was read like any other event and flipped the
  session back to `COMPLETE`, `THINKING`, or `ERROR`. `Registry::apply_hook`
  (`registry.rs`) decides list membership from the state left after
  applying an event, so a straggler that got through re-listed a session
  that had already ended. Every event but a session-start (a resume, which
  legitimately revives an ended session) is now ignored outright once a
  session is `ENDED`. `docs/ARCHITECTURE.md` records the rule alongside the
  other events that read like state changes and are not.
- **A row click resolves by session id, not row index, and Reveal no
  longer blocks the surface.** Clicking a row used to fire two IPC calls,
  `select_tile(index)` then `reveal_session(index)`, each resolving the
  same numeric row index at a different moment; a row above the clicked
  one ending between the two calls could select or reveal the wrong
  session. One `activate_session({ sessionId })` command replaces both:
  `Registry::begin_activation` (`registry.rs`) resolves the id itself
  under a short lock scope, returning an explicit miss with no
  substitution and no selection when the id is no longer bound, or an
  owned `RevealRequest` otherwise. Reveal itself (which can take a few
  seconds) used to run synchronously inside the command on the
  webview's own IPC/event thread, freezing every click, drag, and Quit
  for as long as it took; it now runs on one dedicated worker thread
  (`reveal_queue.rs`), which also guarantees an older, slower reveal
  can never raise a window after a newer one has already completed.
  `activate_session` is now `async` and awaits the worker's reply via
  `spawn_blocking` without blocking the event thread; a worker panic or
  dropped reply channel resolves as an ordinary visible miss ("Reveal
  did not finish.") rather than a silently vanishing rejected invoke.
  The surface's row notes (`rowNotes`, `main.ts`) are keyed by session
  id for the same reason, so a delayed miss always lands on the row it
  was raised for.

### Security

- **Ingest hardening: a CSP, a body cap, a constant-time token compare,
  and a validated Reveal launch folder.** The surface's webview now
  loads under a Content Security Policy in `tauri.conf.json`
  (`default-src 'self'; script-src 'self'; style-src 'self'; font-src
  'self'; img-src 'self'; connect-src ipc: http://ipc.localhost`);
  nothing in the surface needed a looser policy. The daemon's loopback
  ingest endpoint (`http.rs`) now caps a request body at 8 MiB,
  answering `413` and dropping the event outright, never partially
  applied, over that limit; refuses chunked bodies unread with `411`,
  since the chunked decoder's framing buffer sits beneath that cap; and
  compares the per-start ingest token in
  constant time instead of with a short-circuiting `==`. Reveal's VS
  Code path (`reveal.rs`) now refuses to launch `code.cmd` unless the
  workspace folder it read from `~/.claude/ide/*.lock` is an absolute
  path to a directory that actually exists, and every `Cargo.toml` in
  `app/` and `shim/` now pins `rust-version = "1.77.2"`, closing
  CVE-2024-24576. `docs/SECURITY_MODEL.md` records the change and the
  residual risk it does not close: any process running as the same OS
  user can still read the ingest token and spoof hook events, tracked
  in `TODO.md` to close before Phase 2.

### Changed

- **Reveal classifies the session's host and treats a tie as a miss,
  recorded as ADR-032.** Fixes a real "No window matched" the owner hit:
  a subagent hook payload's own `cwd` (it shares its parent session's
  `session_id` but carries a different directory) was overwriting the
  session's real working directory, poisoning every later Reveal for it.
  `apply_hook` (`state.rs`) now skips `cwd` and the label derived from
  it on any payload carrying `agent_id`, and `register_enumerated`
  (`registry.rs`) corrects an existing `cwd` that disagrees with
  `claude agents --json`'s own value rather than only filling in a
  blank one, so an already-poisoned session repairs itself on the next
  enumeration pass. Separately, Reveal no longer resolves a tie at the
  top score by pick order: two or more windows tied for the best score
  now report the same honest miss as no match at all. Reveal also stops
  applying one scored match to every host: it classifies the session's
  pid first, by walking its parent chain (a Toolhelp32 snapshot, at
  most eight hops) into `Code.exe` (VS Code), `WindowsTerminal.exe`
  (Windows Terminal), or a plain console, and picks a strategy per
  host. A console is matched exactly with `AttachConsole` plus
  `GetConsoleWindow`, falling back to the scored title match if the
  attach fails (unverified against a live console session). Windows
  Terminal raises its one open window, or reports an honest miss
  naming the ambiguity when more than one is open, since no interface
  can target a specific tab from outside the process
  (`microsoft/terminal#19783` was closed not planned in January 2026).
  VS Code reads `~/.claude/ide/*.lock` for the `pid` and
  `workspaceFolders` fields only (never its `authToken`, never the
  WebSocket MCP server the file also advertises), matches the
  session's `cwd` against those folders (an ancestor tie is ambiguous,
  same as any other tie), runs VS Code's own CLI against a matched
  folder from the host's own resolved install directory (never `PATH`,
  never anything the session controls), then restricts the title raise
  to Code.exe-owned windows naming that folder, falling back to the
  pre-existing pid-blind title match when nothing above resolves it.
  The extension's own session-tab link is deliberately left unwired:
  reading `extension.js` (2.1.270) shows it would risk opening a
  second, duplicate session rather than revealing the first one in at
  least two situations neither hooks nor `claude agents --json` can
  currently rule out. `anthropics/claude-code#77827` (a terminal
  refocus captured as a click on a permission prompt) is recorded as a
  risk of Deckhand's click-to-raise, not mitigated. `docs/DECISIONS.md`,
  `docs/CONTROL_MAPPING.md`, `docs/ARCHITECTURE.md`,
  `docs/SECURITY_MODEL.md`, `docs/CLAUDE_CODE_ADAPTER.md`, `TODO.md`,
  and `CLAUDE.md` are updated to match, and `app/` implements the
  change.
- **Move is removed and the header becomes a drag bar, recorded as
  ADR-031.** The Move button and the `cycle_position` command it drove
  are deleted; the window is now repositioned by dragging only, an
  owner-approved exception to the no-required-drag rule in
  `docs/ACCESSIBILITY.md`, recorded there as an open accessibility gap,
  not as compliance. The whole header (`#header`) is now the drag
  region, with the state-count pills passing pointer events through so
  dragging on them drags too; the striped drag grip is gone. The header
  shrinks to 52 px (was 64), 4 px padding, ordered state counts, grey
  toggle, Quit; Quit is a 44 by 44 px icon-only button (a cross glyph,
  `aria-label="Quit Deckhand"`). The window's initial height in
  `tauri.conf.json` follows, 116 px. The grey toggle is relabelled to
  name what it acts on: "Hide unknown" when rows show, "Show N unknown"
  when hidden ("Show unknown" if N is 0, via a new `greyLabel()` helper
  in `format.ts`, tested); its pressed style becomes a lighter
  background and brighter text instead of an inset outline, and it
  disappears entirely when there is nothing unknown and hiding is
  already off, so Quit never moves. The all-hidden placeholder row now
  reads "N unknown hidden." Header buttons are otherwise plain text
  buttons: 44 px minimum height, 12 px side padding, 6 px radius, 14 px
  bold. The dashed outline unknown and ended rows shared is removed;
  they are set apart by glyph shape and dimming alone, which
  `styles.test.ts` now pins directly. A Reveal miss note moves onto the
  row's second line, one short line to the right of the state word
  (`revealNote()` in `format.ts`, tested), instead of a multi-line side
  column that used to squeeze the name, clip the state word, and grow
  some rows past 64 px. `docs/DECISIONS.md`, `docs/CONTROL_MAPPING.md`,
  `docs/UI_SPEC.md`, `docs/ACCESSIBILITY.md`, `docs/ARCHITECTURE.md`,
  `README.md`, `CLAUDE.md`, and `TODO.md` are updated to match, and
  `app/` implements the change.
- **The header gains a Hide grey toggle, recorded as ADR-030.** Order is
  now drag grip, read-only state counts, Hide grey, Move, Quit: three
  header controls, not two. A single click flips the setting. Off, the
  control reads "Hide"; on, it shows pressed (a 2 px inset outline) and
  reads "Show N," N being the count of rows currently in the `unknown`
  state (both "not heard yet" and past `T_unknown`), so a hidden session
  is always counted, never silently gone. If every bound session is
  hidden, the list shows one placeholder row, "N grey hidden." The
  header's count pills are unaffected and are tightened to make room for
  the new control. Glyph: the unknown state's grey question mark. The
  daemon owns the setting (`Registry.hide_unknown`), persists it in a
  new `settings.json` alongside `window.json` and `bindings.json`
  (a missing field or a corrupt file loads as `false`), sends it to the
  surface as `hideUnknown` in the snapshot, and exposes it as the
  `toggle_hide_unknown` Tauri command. The window now sizes from the
  visible row count (`visible_row_count` in `window.rs`), including when
  the `T_unknown` watchdog moves a hidden session into `unknown`. Known
  trade-off: because `heard` (ADR-029) resets on every restart, a session
  that was genuinely waiting on the owner before the restart also renders
  `unknown` until a hook fires for it again, so hiding grey can hide a
  session that needs a human; "Show N" is the accepted mitigation, not a
  fix. `docs/CONTROL_MAPPING.md`, `docs/UI_SPEC.md`,
  `docs/ACCESSIBILITY.md`, and `docs/ARCHITECTURE.md` are updated to
  match, and `app/` implements the change.
- **The session list gets taller rows, header counts, and a bundled
  typeface, recorded as ADR-029.** Rows grow to a fixed 64 px, two lines
  (the session name over the state word), with a 30 px glyph and a
  background tint keyed to the state's colour: idle 6%, thinking and
  complete 14%, needs input and error 22%. Unknown and ended get a dashed
  outline instead of a tint, and a selected row gets a 3 px inset outline
  in the text colour. A Reveal miss note moves into its own column at the
  right of the row. The header grows to 64 px and gains a read-only
  summary of counts (waiting on you, error, thinking, complete) between
  the drag grip and the still-only-two controls, Move and Quit. The
  surface's type changes from the system Segoe UI to a bundled Atkinson
  Hyperlegible Next (latin subset, SIL OFL 1.1), falling back to Segoe UI
  outside that subset. The empty list now reads "Watching for sessions"
  instead of "No sessions." Unknown rows now say which of two things
  happened: the daemon's `Session` gains `heard: bool`, set once any hook
  event has arrived for it in this run, and the row reads "not heard yet"
  while `heard` is false (bound by enumeration or restored from disk, no
  hook seen yet this run) or "unknown" once heard from and then quiet past
  `T_unknown`, same colour and glyph either way (ADR-008 unchanged).
  Unknown rows also dim: glyph and state word to 75% grey, name to regular
  weight at 72% text colour, short of ended's dimming. The window's
  initial height in `tauri.conf.json` follows, 128 px. `docs/UI_SPEC.md`,
  `docs/ACCESSIBILITY.md`, `docs/ARCHITECTURE.md`,
  `docs/CLAUDE_CODE_ADAPTER.md`, and `TODO.md` are updated to match, and
  `app/` implements the change.
- **The surface narrows to a session list, recorded as ADR-028.** An
  ordered, unbounded list replaces the six fixed tiles: one row per
  session (colour, glyph, name, state word), click to select and raise
  (ADR-027 unchanged), and a header holding only Move and Quit. Removed
  from the plan: the command keys (approve, deny, answer, interrupt,
  continue, reveal), the stick, the dial, talk and send, the detail
  panel, the bind picker, the layer strip, and the two corner badges.
  Approve and deny stay Phase 2 work; landing them, or anything else, on
  this surface now needs its own ADR. Binding becomes automatic: a
  session is bound on its first hook event or `claude agents`
  enumeration hit, the daemon reruns that enumeration every 15 seconds
  outside the registry lock, and a session is dropped on ending or on
  going 60 seconds with no hook after a successful enumeration stops
  listing it; a failed enumeration prunes nothing, and a legacy
  six-slot `bindings.json` loads by dropping its null slots. The window
  becomes a vertical list about 360 px wide, sized to the row count at
  48 px per row, clamped to the monitor work area, with a saved
  position validated against the monitors actually connected at
  startup, and the raise now excludes Deckhand's own window from its
  candidates. `docs/CONTROL_MAPPING.md`, `docs/UI_SPEC.md`,
  `docs/ARCHITECTURE.md`, `docs/ACCESSIBILITY.md`,
  `docs/EXECUTIVE_SUMMARY.md`, `README.md`, `TODO.md`, and `ROADMAP.md`
  are updated to match, and `app/` implements the list.

### Added

- A proposed
  [OpenAI integration implementation plan](docs/OPENAI_INTEGRATION_PLAN.md)
  with requirements, evidence limits, adapter and identity changes,
  migration, security gates, work packages, and validation criteria.
  Codex observation is the proposed first target; no OpenAI adapter or
  additional control capability is implemented by this planning change.

- A test suite across all three parts. The daemon crate gained a
  `lib.rs` so tests can reach its modules; window matching, the
  enumeration parser, and persistence were split at pure seams and
  covered, the loopback ingest endpoint is exercised over a real socket,
  and the state machine and registry tests grew. A headless pipeline
  test drives six sessions through the real endpoint into six tiles of
  six different colours, which is the six-session colour test from
  TODO.md without the screenshots. The shim has black-box tests that
  spawn the built binary against a fake daemon and pin its silence and
  its fail-open exits. The surface's pure helpers moved to their own
  module and run under `node:test` with no new dependencies, including
  a check that every state has a glyph and a stylesheet rule.
  `.github/workflows/tests.yml` runs all of it on Windows, and
  `scripts/build-app.ps1` runs it locally.

- `run.py` at the repo root: check the toolchain with install hints,
  build, optionally test, and restart the board in one command
  (`--check`, `--test`, `--no-build`, `--stop`), and a
  documentation sweep that moved every stale "Phase 0, no code" claim
  across the README, roadmap, changelog intro, executive summary,
  architecture, and community files to the Phase 1 reality. Reveal
  attempts now also log what they searched for and what won to
  `%LOCALAPPDATA%\deckhand\reveal.log`, because the owner's first live
  try did not visibly work and the next report should be diagnosable.

- The rest of the control surface, so the strip matches the design
  instead of stopping at six tiles: the six command keys (Approve, Deny,
  Answer, Interrupt, Continue, Reveal), the stick (scroll, panel toggle,
  previous tile; drawn as a 2 by 2 grid until the diamond geometry is
  built), the dial as a disabled readout, Talk and Send placeholders,
  and the detail panel (identity, state in words, current item, question
  options, Reveal, Unbind, Scan). Disabled controls follow the spec
  rule: visible, dimmed, and clicking one puts the honest reason in the
  panel, never a silent no-op. Reveal actually acts: a pid-then-title
  window match raises the selected session's host window. The window
  gained a drag grip, a Move key that cycles screen-edge presets so
  moving never requires a drag, and its position persists across
  restarts.

### Changed

- A tile click now selects the session and raises its host window in
  the same click, recorded as ADR-027. The separate Reveal target on
  the tile and the double-click accelerator are gone; the Reveal key
  and the panel action repeat the raise for the selected session.
  Deckhand's own window still never takes focus.

- Live validation against Claude Code 2.1.220, recorded as ADR-026, put
  real sessions through the Phase 1 pipeline and corrected the spec and
  code in four places: `PostToolUseFailure`'s observed shape (`error`
  string plus `is_interrupt`, no `error_type`; an interrupt closes every
  open operation), the child ledger now keyed on the observed `agent_id`
  so duplicate delivery is a no-op and a stray stop cannot close an
  unrelated bracket, `claude agents --json` treated as successful on
  parseable output because it exits 255, and `permission_mode:
  "default"` added to the protocol as an observed payload value. Nine of
  twelve documented hook events have now fired live. Tile text also
  became legible: tiles no longer inherit the button default black, and
  the slot and badge sizes came up.

### Added

- The Phase 1 observation skeleton, the first application code beyond the
  spike. One Tauri application in `app/` holds the daemon (session
  registry, the state machine from
  `docs/CLAUDE_CODE_ADAPTER.md#status-inference` with the table encoded
  as unit tests, loopback HTTP ingest with a per-start token per ADR-007,
  cold-start enumeration per ADR-024, bindings persisted across restarts)
  and the TypeScript tile surface (six tiles, triple-coded state, drawn
  glyphs, mode and children badges, bind picker, 44 px floor, no keyboard
  handlers, the ADR-025 no-focus-steal mechanism). `shim/` is the
  std-only hook shim: stdin to POST, always exits 0 and silent, so a dead
  daemon can never block a session. Nothing in Phase 1 holds any write
  authority. Built by `scripts/build-app.ps1`; proven end to end by
  `scripts/phase1-smoke.ps1`, which drives synthetic hook events through
  the real shim and screenshots the painted tiles.

- The first application code: the pre-Phase-1 window spike at
  `spikes/tauri-focus/`, a minimal Tauri v2 app proving the always-on-top,
  no-focus-steal window on Windows 11, with an automated check in
  `scripts/focus-test.ps1`. Result recorded as ADR-025: `alwaysOnTop`
  supplies `WS_EX_TOPMOST` but not `WS_EX_NOACTIVATE`; one
  `SetWindowLongPtrW` call at setup adds it, and a click into the webview
  then registers without the window ever taking the foreground. Half the
  ADR-009 gate closes; hook payload validation stays open.
- A payload capture hook, `.claude/hooks/payload-capture.js`, registered
  in `.claude/settings.json` for all twelve documented hook event names:
  every event is appended raw to gitignored `_scratch/hook-capture.jsonl`,
  fail-open, observe-only. Five captured `PreToolUse` events moved the
  common payload fields (`session_id`, `cwd`, `transcript_path`,
  `prompt_id`, `permission_mode`, `effort.level`, `tool_use_id`) from
  `documented` to `observed` against 2.1.220 in
  `docs/CLAUDE_CODE_ADAPTER.md`; the remaining events enumerate
  themselves passively as future sessions run.
- A change-propagation row in `docs/WORKFLOW.md` for upgrading or
  correcting a verification stamp, matching how ADR-024 and ADR-025
  actually propagated.

- Nine new ADRs, 013 to 021, in `docs/DECISIONS.md`: amber's `kind` and
  answer targets (013); the narrow default gate (014); the gating hook's
  restricted output (015); liveness by open-operation bracketing (016);
  `claude agents --json` as a second observation channel, superseding part
  of ADR-005 (017); permission mode as a first-class axis (018); a tile
  budget with `COMPLETE` waiting on the child ledger (019); attached-mode
  send as unproven rather than impossible, refining ADR-004 (020); and no
  tooltip-only reveals (021). ADR-006 and ADR-008 stand unchanged by all
  nine.
- A partial verification stamp for `docs/CLAUDE_CODE_ADAPTER.md`, replacing
  the blanket "none yet". Checked 2026-07-30 against Claude Code 2.1.220:
  `claude agents --json`, the captured status line payload keys, and the
  `~/.claude/projects/` directory mangling are marked `observed`. Everything
  else, including hook names and the permission decision vocabulary, stays
  `documented` or `unverified` and is named as such; the Phase 1
  verification spike stays open, because a string read from documentation
  is not an observation.
- `SessionUpdate.detail.kind: "permission" | "question"`. Approve and Deny
  now enable only for `kind: "permission"`; a question renders one answer
  target per option, full label, never a bare letter or index, at the 44 px
  floor with the mandated dead gap. `kind: "question"` is raised by a
  `PreToolUse` for `AskUserQuestion`, which is why the hook block installs
  a second, non-gating `PreToolUse` entry: the narrow gate would never see
  it. An amber whose kind cannot be determined, a bare `Notification` for
  instance, carries no kind at all and lights neither control. The
  capability behind Answer, `answer_question`, is declared optional and
  unproven: no answer channel has been observed, so Deckhand can show the
  question and not answer it. No new colour or state; amber is still amber.
- A fixed tile budget of four content slots plus two corner badges
  (permission mode, live child count), ranked by value per pixel, plus the
  child ledger itself: `COMPLETE` is now unreachable while any
  `SubagentStart` has not been matched by a `SubagentStop`.
- Status lines to the five design docs that were missing one:
  `docs/ACCESSIBILITY.md`, `docs/CONTROL_MAPPING.md`, `docs/DECISIONS.md`,
  `docs/WORKFLOW.md`, and `docs/EXECUTIVE_SUMMARY.md`.
- An **ARCH** column (`docs/ARCHITECTURE.md`) added to the
  change-propagation table in `docs/WORKFLOW.md`, with a row for changes to
  the session state machine, plus rows for timing and timeout changes, hook
  event set changes, and control label or wording changes.
  `docs/EXECUTIVE_SUMMARY.md` is now listed in the source-of-truth map,
  marked derived and authoritative for nothing.
- Design mockups in `assets/` (the surface, the approval card, the seven
  state tiles), drawn as SVGs directly from `docs/UI_SPEC.md` and embedded
  in the README with full alt text. They are labelled as mockups, not
  screenshots, because nothing runs yet and the README should not imply
  otherwise.
- Community scaffolding on GitHub: `accessibility`, `adapter`, `spec`, and
  `spike` labels; seven seeded issues covering the two Phase 1 gating
  spikes, the open research questions, and two good first issues; and a
  welcome post in Discussions explaining how to help at Phase 0.
- Initial specification for Deckhand: a mouse-only, always-on-top macropad
  surface for Claude Code sessions, reimplementing the interaction model of
  the Codex Micro (Work Louder and OpenAI) against Claude Code instead of
  the ChatGPT desktop app. The specification covers the control surface
  (six agent tiles, six command keys, a four-way stick, a dial, a talk
  button, a send button, and a layer strip) and the status colour model
  inherited from the device.
- The attached and hosted mode split, and the reasoning behind it: attached
  mode watches sessions the user started themselves and gets full status
  observation and approve and deny authority through hooks, but has no
  proven way to put a prompt into a running interactive session, since the
  channels Claude Code documents all deliver at a turn boundary and none
  has been observed here; hosted mode starts sessions through the Claude
  Agent SDK and gets full control, including send, at the cost of the
  normal terminal UI. This split is documented as the central
  architectural fact of the project, because it determines what every
  other document has to account for.
- The security posture for the approve and deny path: because the
  `PreToolUse` hook can return an actual permission decision, the approve
  button is documented as a security surface from the outset rather than a
  convenience, with `docs/SECURITY_MODEL.md` written to keep that honest as
  the surface grows.
- The accessibility premise: mouse-only operation is the reason the project
  exists, not a feature added afterwards. `docs/ACCESSIBILITY.md` and the
  accessibility section of `CONTRIBUTING.md` exist to keep that requirement
  from eroding as controls get added later.
- Repository process documents: `ROADMAP.md` (the phased plan), `TODO.md`
  (open work), `IDEAS.md` (an unfiltered scratchpad), and
  `docs/WORKFLOW.md` (which document is authoritative for which fact, and
  what has to change alongside what).

### Changed

- **The host is now a third axis, and capabilities belong to a session**
  (ADR-023). Mode said who started a session and was quietly assumed to
  mean "in a terminal"; sessions run inside the VS Code extension too, and
  that host has a different capability set on the same adapter.
  `SessionInfo` gains `host` (`pty`, `vscode-extension`, `sdk`) and carries
  its own `capabilities`; the adapter's record becomes a ceiling rather
  than a promise. `docs/ADAPTER_PROTOCOL.md`, `docs/ARCHITECTURE.md`,
  `docs/CLAUDE_CODE_ADAPTER.md`, `docs/CONTROL_MAPPING.md`,
  `docs/SECURITY_MODEL.md`, and `docs/EXECUTIVE_SUMMARY.md` follow. The
  headline finding is that nothing hooks provide varies by host, so status,
  approve, deny, the mode badge, and the bind picker are unchanged; only
  the controls that need a window differ.
- Reveal no longer claims pid matching works everywhere. Every VS Code
  window shares one process, so a pid identifies none of them (three live
  windows, one pid, observed 2.1.220). On a `vscode-extension` host,
  window-title matching is the only route, and it raises the window without
  selecting the session's tab. `docs/CONTROL_MAPPING.md` says so.
- The verification stamp in `docs/CLAUDE_CODE_ADAPTER.md` gains two
  observations and loses a wrong one. Gained: a `PreToolUse` hook seen to
  fire with `tool_name` and `tool_input` populated and its
  `permissionDecision: "deny"` honoured, which is the first hook observed
  firing in this project at all; and the shape of the VS Code extension
  host, including the per-window MCP server at `~/.claude/ide/<port>.lock`,
  its twelve tools, and the fact that `openFile` with `makeFrontmost` moves
  a tab but not the OS foreground window. Lost: the claim that
  `claude agents --json` returns a `status` key, which no row carried on
  the 2026-08-02 re-run. ADR-009 still gates Phase 1: one event with two
  fields confirmed is not payload validation.
- Reframed the product around answering, not only approving. README and
  `docs/EXECUTIVE_SUMMARY.md` now lead with seeing every session, answering
  the questions it asks, and approving the calls that need a human; Approve
  and Deny stay, but are no longer the headline. The reason is one measured
  corpus on one machine (240 sessions, 2026-07-30): 322 `AskUserQuestion`
  calls across 155 of those sessions, against 10 to 27 tool denials in the
  same corpus. The docs say plainly that this is one user's habits, not a
  general finding.
- Retabled the six command keys against that same measurement. New
  defaults: Approve, Deny, Answer (new), Interrupt, Continue, and Reveal
  (renamed from Raise window). Plan mode and Compact are demoted to the
  detail panel, two `ExitPlanMode` events in 240 sessions and both
  model-invocable, with no overflow shelf added to hold them. Continue
  ships disabled with a stated reason in attached mode, since Deckhand has
  no send channel there yet. `docs/CONTROL_MAPPING.md`, `docs/UI_SPEC.md`,
  and `docs/ADAPTER_PROTOCOL.md` now agree on the table.
- Retasked the stick from stepping tiles to scrolling the detail panel (up
  and down) and expanding or collapsing it (right); left still returns to
  the previously selected tile. Six or more concurrent sessions happens
  0.5% of the time, so tile-stepping spent two of four directions reaching
  a tile that was already one click away, while reading a long pending
  tool input before deciding on it had no cheap pointer path at all.
- Narrowed the default `PreToolUse` gate from `matcher: "*"` to an `if`
  condition scoped to shell execution and file deletion. A match-all gate
  on a machine running `auto` mode, where a classifier already answers
  most permission prompts, turned a zero-prompt session into one amber per
  tool call, making Deckhand the cause of the clicks it exists to remove.
  `docs/SECURITY_MODEL.md` gains a "what amber means under a gate"
  paragraph so this is never mistaken for "Claude Code would have asked
  you".
- Made permission mode a first-class, documented axis: `SessionInfo` now
  carries `permissionMode` (the documented modes plus `unknown`, which is
  also what an unrecognised value maps to), shown on the tile as a text
  badge, never a colour. `docs/SECURITY_MODEL.md` and
  `docs/CLAUDE_CODE_ADAPTER.md` say where an `ask` actually lands in each
  mode (a human in `manual`, the classifier in `auto`, a denial in
  `dontAsk`), and record the `auto`-mode classifier as a second gate
  Deckhand does not control.
- Replaced the turn-duration liveness deadline with liveness by open
  operation. A session now stays `THINKING` while any `PreToolUse` lacks a
  matching `PostToolUse` or `PostToolUseFailure`, or any `SubagentStart`
  lacks a `SubagentStop`, and the tile shows elapsed-in-operation instead
  of elapsed-in-state. Measured p90 turn duration is 660 seconds, which
  made any turn-duration deadline false-grey healthy sessions at every
  useful value. There is now one deadline, `T_unknown` (900 second
  default); the second stale tier and its badge are gone.
- Corrected the hook installation block in `docs/CLAUDE_CODE_ADAPTER.md`:
  removed `"timeout": 5` from `SessionEnd` (those hooks share a 1.5 second
  budget), added `"async": true` to every non-gating entry, and added
  exactly the events the state machine now depends on
  (`PostToolUseFailure`, `StopFailure`, `PermissionDenied`, and
  `SubagentStart` and `SubagentStop` for the child ledger and the liveness
  bracket) plus a second, non-gating `PreToolUse` entry, rather than
  jumping to the full set of 30-plus documented events. Thirteen entries
  across twelve events. Recorded that
  Deckhand writes only to the user-level `~/.claude/settings.json`, never
  the git-shared project settings file.
- Restricted the gating hook's output to exactly `hookEventName`,
  `permissionDecision`, and `permissionDecisionReason`. `updatedPermissions`
  and `updatedInput` are now explicitly forbidden: a durable permission
  write would be invisible to every later amber and outside the
  attribution guarantees `docs/SECURITY_MODEL.md` already makes, and
  Deckhand does not edit tool inputs.

### Fixed

- A stale `observed` stamp on the `status` key of `claude agents --json`,
  which survived in `docs/ARCHITECTURE.md` and in the cold-start section of
  `docs/CLAUDE_CODE_ADAPTER.md` after the 2026-08-02 re-run found no row
  carrying one. The adapter file contradicted its own verification stamp
  eight lines from the top. Recorded as ADR-024, which narrows ADR-017
  without editing it: `claude agents --json` stays a load-bearing
  enumeration channel, but at cold start it recovers a session's binding and
  label, not its state. The `busy` to `THINKING` mapping is kept as a
  conditional and currently never fires. ADR-017's own reopen condition,
  "this reopens if the output shape changes", is what triggered the entry.
  No colour was ever wrong as a result, because a missing status already
  mapped to `UNKNOWN` rather than to `IDLE`; the defect was an overstated
  claim, not a wrong tile.
- The `startedAt` type in the cold-start section of
  `docs/CLAUDE_CODE_ADAPTER.md`. It arrives from `claude agents --json` as
  epoch milliseconds, and `docs/ADAPTER_PROTOCOL.md` types `SessionInfo`
  `startedAt` as an ISO 8601 string, so the conversion is now stated
  instead of assumed.
- Three defects in the new tooling, found while reviewing it. The
  `.claude/hooks/style-gate.js` wrap warning fired at 88 columns while
  `scripts/check-docs.ps1` reports at 80, so the pre-flight and CI
  disagreed about the same line; the gate's exemption pattern matched a
  bare filename suffix where the script compares the leaf filename, so
  something like `OUR_CODE_OF_CONDUCT.md` would have been exempt locally
  and failed in CI; and gate 4 of the script read list items inside fenced
  code blocks and left a level-4 heading in whatever section state
  preceded it. The gate's header now also says plainly that it is a
  pre-flight rather than the enforcement point, and that it sees `Write`
  and `Edit` only, so markdown written through a Bash heredoc reaches CI
  unchecked.
- The permission mode set, wrong in both directions one commit ago:
  `default` was listed as a mode and `manual` was missing, making seven
  values where there are six. Running `claude --help` on Claude Code 2.1.220
  observed the six as `acceptEdits`, `auto`, `bypassPermissions`, `manual`,
  `dontAsk`, and `plan`. Recorded as ADR-022, correcting ADR-018 without
  editing it; whether `permissions.defaultMode` accepts `default` stays
  unverified.
- Two provable wrong-colour bugs in status inference: `SessionStart` with
  `source: "compact"` no longer flips a live blue tile white, because
  compaction fires mid-turn and changes nothing; and `SessionEnd` with
  `reason: "clear"` or `"resume"` no longer reports a session `ended`,
  because each is followed by a new `SessionStart` for the same terminal.
- The accessibility violation of tooltip-only reveals. The five
  load-bearing tooltips in `docs/UI_SPEC.md` and `docs/ADAPTER_PROTOCOL.md`
  are replaced with click-to-reveal into the detail panel: clicking a
  disabled control is never a no-op, and the click expands the panel first
  if it is collapsed. Recorded as ADR-021.
- The flat, false claim that Claude Code has no supported way for an
  external process to put a prompt into a running session. Replaced with
  what actually exists and its shape: Stop-hook `decision: "block"`,
  `SessionStart` `initialUserMessage`, and `additionalContext`, none of
  which delivers into an idle session. `send_prompt` stays `false` in
  attached mode, and a Phase 1 spike to observe Stop-hook block behaviour
  was added to `ROADMAP.md`.
- The protocol type `PermissionRequest` in `docs/ADAPTER_PROTOCOL.md`,
  renamed to `DeckhandPermissionRequest`. The old name collided with a real
  Claude Code hook event of a different shape.
- This changelog's own earlier claim, below, that the PR template's sync
  checklist "covers all eight authoritative documents". At the time that
  was written, the checklist covered seven; `docs/ARCHITECTURE.md` was
  missing from it. That gap is now closed by the new ARCH column above, so
  the claim below is corrected to describe what actually happened rather
  than restated as if it had always been true.
- Documentation sweep across the whole specification, from two independent
  audit passes (consistency and honesty). The one real design contradiction
  found: three documents disagreed on what a dead session process turns a
  tile into. Resolved as: a clean exit is `ended`, a confirmed death
  without a clean exit is a crash and is `error` (acknowledging it then
  shows `ended`), and silence past the liveness deadline stays `unknown`.
  `docs/ARCHITECTURE.md`, `docs/CLAUDE_CODE_ADAPTER.md`, and
  `docs/CONTROL_MAPPING.md` now say the same thing.
- Smaller alignment fixes: the adapter request template now lists all
  eight capabilities in the protocol's order, `answer_question` included;
  the PR sync checklist, at
  the time this line was first written, covered seven authoritative
  documents, not the eight it claimed (corrected above, now that
  `docs/ARCHITECTURE.md` has genuinely joined the checklist); the
  source-of-truth map gained rows for `docs/ARCHITECTURE.md` and
  `docs/ACCESSIBILITY.md`; `TODO.md` no longer lists as open three
  decisions that ADR-007 and the adapter doc had already made; state
  tables use one row order and one name per concept (`needs_input`,
  pinned mode); phase labels for the two de-risking spikes agree (Phase 0,
  gating Phase 1); auto-approval rules are unscheduled rather than
  promised for Phase 3; the executive summary and README now hedge
  unbuilt behaviour and carry the non-affiliation note.
