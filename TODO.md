# TODO

GitHub-flavoured checkboxes, organised by the phases in `ROADMAP.md`, plus a
backlog for things not tied to a specific phase yet, and a completed log at
the bottom.

Checking a box here means the item is done, not that it is perfect. See
`docs/DECISIONS.md` for why something was decided a certain way, and
`docs/WORKFLOW.md` for what else needs to change alongside it.

---

## Phase 0: Specification

### Documentation

- [x] `README.md`
- [x] `docs/CONTROL_MAPPING.md`
- [x] `docs/ARCHITECTURE.md`
- [x] `docs/ADAPTER_PROTOCOL.md`
- [x] `docs/CLAUDE_CODE_ADAPTER.md`
- [x] `docs/UI_SPEC.md`
- [x] `docs/SECURITY_MODEL.md`
- [x] `docs/ACCESSIBILITY.md`
- [x] `docs/DECISIONS.md`
- [x] `docs/EXECUTIVE_SUMMARY.md`
- [x] `docs/WORKFLOW.md`
- [x] `CONTRIBUTING.md`
- [x] `ROADMAP.md`
- [x] `TODO.md`
- [x] `IDEAS.md`
- [x] `CHANGELOG.md`
- [x] `SECURITY.md`
- [x] `SUPPORT.md`
- [x] Issue templates (bug report, feature request, accessibility feedback,
      adapter request)
- [x] CI (a lint and link check on push; there is nothing to compile yet)

### Remaining specification work

- [ ] Validate every hook payload field named in
      `docs/CLAUDE_CODE_ADAPTER.md` against a real, current Claude Code
      install. The spec was written from documentation, not a packet
      capture.
      Partially done, against Claude Code 2.1.220 on 2026-07-30. Actually
      run or read on this machine (`observed`): `claude agents --json`, the
      captured status line payload keys, the `~/.claude/projects/`
      mangling, and the `--permission-mode` value set from `claude --help`
      (`acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`,
      `plan`; `default` is not one of them). Read from official
      documentation, not seen to fire here
      (`documented`): the `hookSpecificOutput` wrapper and the
      `allow`/`deny`/`ask`/`defer` vocabulary, `matcher: "*"` plus the `if`
      field, hook timeouts in seconds, and the common payload fields
      including `prompt_id`, `permission_mode`, `effort.level`, and
      `tool_use_id`. Still unverified: hook overhead at six concurrent
      sessions, what the user sees when a hook times out on Claude Code's
      side, how conflicting decisions across two hook entries resolve, any
      behaviour outside `manual` permission mode (the mode names are
      observed, what each mode does is not), and whether the settings key
      `permissions.defaultMode` accepts a value spelled `default` even
      though the CLI flag does not. See
      `docs/CLAUDE_CODE_ADAPTER.md` for the full per-claim stamp.
      Advanced again on 2026-08-02: a `PreToolUse` hook was seen to fire,
      with `tool_name` and `tool_input` populated, and its
      `permissionDecision: "deny"` was honoured and blocked the call. That
      is the first hook observed firing here, and it fired from a session
      running inside the VS Code extension. Also observed that run: no
      `status` key on any `claude agents --json` row, correcting the
      2026-07-30 note. Advanced a third time later on 2026-08-02, when the
      capture tap below logged five complete `PreToolUse` events and moved
      every common field to `observed` for that event. Advanced a fourth
      time the same day by live validation (ADR-026): nine of the twelve
      documented events have now fired here with their fields captured,
      correcting `PostToolUseFailure`'s shape (`error` string plus
      `is_interrupt`, no `error_type`) and adding `permission_mode:
      "default"` as a live payload value. This item stays unchecked until
      `Notification`, `StopFailure`, and `PermissionDenied` are seen
      firing.
- [x] Enumerate the remaining `PreToolUse` payload fields now that the
      event is known to fire. Done 2026-08-02: a capture tap (first inside
      the style gate, now the dedicated `.claude/hooks/payload-capture.js`)
      appends every raw event to gitignored `_scratch/hook-capture.jsonl`,
      and five captured events showed `session_id`, `cwd`,
      `transcript_path`, `prompt_id`, `permission_mode`, `effort.level`,
      and `tool_use_id` all populated (`vscode-extension` host, 2.1.220,
      `Edit` tool calls). The capture hook is registered for all twelve
      documented event names, so future sessions enumerate the other
      events passively; which names never fire is a finding of its own.
- [ ] Confirm the host discriminator ADR-023 assumes. The plan is to read
      the process argv and parent, since `claude agents --json` reports
      `kind: "interactive"` for a VS Code extension session and a terminal
      session alike (observed 2.1.220). Verify that
      `--input-format stream-json` and a `Code.exe` parent are a reliable
      test, including for a session in VS Code's integrated terminal, which
      is a `pty` host inside an editor window and must not be misread as
      `vscode-extension`.
- [ ] Measure hook call overhead with six Claude Code sessions running
      concurrently. If it is not negligible, `docs/ARCHITECTURE.md` needs an
      answer for it, not just a hope.
- [x] Decide the daemon transport: loopback HTTP with a token, recorded as
      ADR-007. Revisit only if the token model proves inadequate.
- [x] Prototype the Tauri always-on-top, non-focus-stealing window on
      Windows 11 before committing further to Tauri for the rest of the UI.
      Done 2026-08-02: `spikes/tauri-focus/` proves it, recorded as
      ADR-025. Tauri's `alwaysOnTop` sets `WS_EX_TOPMOST` but not
      `WS_EX_NOACTIVATE`; one `SetWindowLongPtrW` call at setup adds it,
      and a synthetic click was then received by a button in the webview
      while the foreground window never changed and the spike window never
      activated. Hedges and untested cases are in the ADR.
- [ ] Confirm whether a genuine Claude Code error state (crash, process
      death, the adapter losing the session) can be detected at all through
      hooks, or whether it needs a separate supervisory heartbeat.
- [ ] Decide what happens when two Claude Code sessions share a `cwd`.
      `docs/CLAUDE_CODE_ADAPTER.md` should say.
- [ ] Decide the transcript JSONL fallback's exact trigger condition: when
      Deckhand falls back to reading
      `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`, and how it
      detects that the schema it expects has changed underneath it.
- [x] Write down the minimum hit target size as an actual number, with a
      rationale: 44 px, in `docs/ACCESSIBILITY.md`, echoed in
      `docs/UI_SPEC.md`.
- [ ] Specify the bind picker (what it lists, how a session is chosen, what
      it shows when the adapter's session list is `internal`-confidence).
- [ ] Design the UI flow for presenting the hook block for confirmation.
      The policy is already set in `docs/CLAUDE_CODE_ADAPTER.md`: Deckhand
      writes only on explicit confirmation, composes, never clobbers.
- [ ] Resolve whether `docs/SECURITY_MODEL.md`'s trust boundary needs to
      account for a compromised hook shim, not only a compromised tool
      call.
- [ ] Choose the narrow gate's default pattern set: the `if` condition that
      scopes `PreToolUse` gating to shell execution and file deletion, so it
      stops matching every tool call in `auto` mode. See
      `docs/SECURITY_MODEL.md`.
- [ ] Prove an answer channel for `AskUserQuestion` exists before promoting
      `answer_question` past optional and unproven. No documented interface
      is known to deliver an answer back into a pending question today.
- [ ] Get at least one outside accessibility review of the mouse-only claim
      before Phase 1 starts, not after.

---

## Phase 1: Observation only

Started 2026-08-02. The skeleton lives in `app/` (daemon and surface, one
Tauri application) and `shim/`; `scripts/build-app.ps1` builds it and
`scripts/phase1-smoke.ps1` drives it with synthetic events end to end.

- [ ] Design the daemon's process lifecycle (start on login, restart on
      crash, single instance).
- [x] Implement the hook shim: the small program Claude Code calls, per
      `docs/CLAUDE_CODE_ADAPTER.md`. Done 2026-08-02: `shim/`, std-only,
      reads stdin, POSTs to the daemon's loopback port with the token
      from `%LOCALAPPDATA%\deckhand\daemon.json`, exits 0 silently in
      every failure mode so it can never block a session.
- [ ] Implement `settings.json` hook registration (install and
      uninstall). The dogfood wiring on this machine is hand-written in
      gitignored `.claude/settings.local.json`; the installable,
      user-level version with an uninstall path is still owed.
- [x] Implement daemon ingestion for the hooked event types. Done
      2026-08-02 for all twelve documented events (the count in this
      item used to say seven; the twelve-event set and the reasons are
      in `docs/CLAUDE_CODE_ADAPTER.md#hook-installation`).
- [x] Define the internal session state machine (idle, thinking,
      needs_input, complete, error, ended, unknown) and its transition
      rules, matching `docs/ADAPTER_PROTOCOL.md` exactly. Done
      2026-08-02: `app/src-tauri/src/state.rs`, with the
      status-inference table encoded as unit tests, including the
      compaction and clear/resume no-change rows, the child ledger
      gating green, `T_unknown` to grey never red, and never guessing
      idle.
- [x] Implement the six agent tiles in the surface shell. Done
      2026-08-02: `app/ui/`, triple-coded state (ring, drawn glyph,
      words), mode and children badges, 44 px floor, no keyboard
      handlers anywhere.
- [x] Wire tile colour to daemon session state over the chosen
      transport. Done 2026-08-02: shim to daemon over loopback HTTP with
      a token (ADR-007), daemon to surface over Tauri events in-process.
- [x] Implement the always-on-top, non-focus-stealing window per the
      Phase 0 prototype. Done 2026-08-02 with the ADR-025 mechanism.
      The focus-test harness has not yet been re-run against this
      window, only against the spike's; do that before trusting it.
- [x] Implement manual tile binding, including the unbound (off) state.
      Done 2026-08-02: unbound tiles render dashed with a plus and open
      the bind picker; first-heard sessions auto-fill free tiles. Unbind
      landed the same day in the detail panel.
- [ ] Implement the transcript JSONL fallback path for a missed hook
      event.
- [x] Handle daemon restart without losing which tile is bound to which
      session. Done 2026-08-02: bindings persist with labels, and a
      bound session the daemon has not seen renders as the right tile,
      rightly named, in grey, never as unbound.
- [ ] Write a manual test script that induces every status colour across
      six concurrent sessions, and decide what gets logged and at what
      verbosity. `scripts/phase1-smoke.ps1` induces five states across
      three tiles and screenshots them; the six-session version and the
      logging decision are still owed.

---

## Phase 2: Approve and deny

- [ ] Implement `PreToolUse` hook response with an actual permission
      decision, and the daemon-side hold while that decision is pending.
- [ ] Wire the approve command key to the held decision.
- [ ] Wire the deny command key to the held decision.
- [ ] Implement a timeout policy for an unanswered permission decision.
- [ ] Implement the audit trail of approvals and denials.
- [ ] Test the failure mode where the daemon dies while a decision is
      pending.
- [ ] Confirm the amber state can only ever be shown when a decision is
      genuinely pending, with a test for the inverse.
- [ ] Security review of the approve and deny path specifically (see
      `docs/SECURITY_MODEL.md`).

---

## Phase 3: The rest of the surface

- [ ] Implement the dial: step through options.
- [ ] Implement the dial: minus and plus targets.
- [ ] Implement the dial: commit target.
- [ ] Implement the four-way stick: scroll the detail panel up and down,
      expand or collapse it, and return to the previously selected tile. No
      tile stepping. All four functions work as of 2026-08-02; the
      rendering is a 2 by 2 grid rather than the specified diamond, so
      this stays open until the geometry matches UI_SPEC.
- [ ] Implement the detail panel the stick opens. A working subset landed
      2026-08-02: identity line, state in words, current or pending item,
      question with disabled answer targets, the reveal-reason landing
      area, Reveal, Unbind, and Scan. Context bar, cost, plan mode,
      compact, and per-session settings are still owed.
- [ ] Implement continue and interrupt command keys. Both render, both
      disabled with their honest reasons; there is no channel for either
      yet (ADR-020).
- [ ] Implement the Answer command key and the per-option answer targets,
      full option labels, disabled until `answer_question` is proven.
      Rendered and disabled as of 2026-08-02, with full labels.
- [ ] Implement the Reveal command key and its pid-based host match.
      Implemented 2026-08-02 as a pid-then-title heuristic
      (`app/src-tauri/src/reveal.rs`) behind the Reveal key and the
      panel action, with the ALT-tap foreground workaround. Stays
      unchecked until a live click is seen to raise the right window.
- [ ] Implement plan mode and compact in the detail panel, not as command
      keys.
- [ ] Implement the layer strip and profile switching.
- [ ] Implement the settings surface.
- [ ] Implement theming.
- [ ] Confirm every control in this phase against the accessibility minimum
      hit target.

---

## Phase 4: Hosted mode

- [ ] Integrate `claude-agent-sdk` for session start and lifecycle.
- [ ] Implement send for hosted sessions.
- [ ] Implement the attached-versus-hosted visual distinction in the
      surface.
- [ ] Decide how hosted session output is displayed, given there is no
      terminal.
- [ ] Cost and rate limit handling for hosted sessions.

---

## Phase 5: Talk

- [ ] Define the MacroVox integration contract.
- [ ] Implement the talk button (click to start, click to stop).
- [ ] Implement the double-press hands-free toggle.
- [ ] Implement graceful degradation when MacroVox is not running.
- [ ] Route transcribed text into the composed message.

---

## Phase 6: Beyond Claude Code

- [ ] Choose the second adapter target.
- [ ] Implement it against the existing `docs/ADAPTER_PROTOCOL.md`.
- [ ] Record every point where the contract had to bend, and fix the
      contract rather than the adapter where possible.

---

## Backlog

Not tied to a specific phase yet.

- [ ] Cross-platform testing on macOS.
- [ ] Cross-platform testing on Linux.
- [ ] Installer, packaging, and update story.
- [ ] Localisation (not started, not committed to).
- [ ] Performance budget for the always-on-top surface (CPU and memory at
      idle).
- [ ] Decide on telemetry (default: none) and document it if that ever
      changes.

---

## Completed

- [x] Repository created and initialised, with an MIT licence.
- [x] Community health files added (`CODE_OF_CONDUCT.md`, `SUPPORT.md`).
- [x] `.gitignore` and `.gitattributes` configured, including the
      `_scratch/` convention for gitignored AI temp files.
- [x] Issue template set added under `.github/ISSUE_TEMPLATE/`.
