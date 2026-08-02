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

- `scripts/run.py`: build and restart the board in one command
  (`--no-build` to skip the build, `--stop` to stop it), and a
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
