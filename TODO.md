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
      "default"` as a live payload value. A fifth advance later that day:
      `PermissionDenied` fired live (the auto-mode classifier blocking a
      call) carrying the common fields plus `tool_name`, `tool_input`,
      `tool_use_id`, and `reason: "Blocked by classifier"`, exactly as
      predicted. This item stays unchecked until `Notification` and
      `StopFailure` are seen firing.
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
- [ ] Review and implement the staged
      [architecture hardening proposal](docs/ARCHITECTURE_HARDENING_PLAN.md):
      duplicate-safe operation tracking, a production mutation controller,
      explicit scan completeness, stable identity and view contracts,
      isolated reveal execution, and bounded lifecycle and I/O. The plan
      records acceptance tests and the newly reproduced duplicate-child
      completion defect; this item is not an implementation claim.
- [x] Implement the hook shim: the small program Claude Code calls, per
      `docs/CLAUDE_CODE_ADAPTER.md`. Done 2026-08-02: `shim/`, std-only,
      reads stdin, POSTs to the daemon's loopback port with the token
      from `%LOCALAPPDATA%\deckhand\daemon.json`, exits 0 silently in
      every failure mode so it can never block a session.
- [x] Implement `settings.json` hook registration (install and
      uninstall). Done 2026-09-13: `scripts/install-hooks.ps1` merges
      the twelve-event, non-gating wiring into the user-level file, is
      idempotent, and `-Uninstall` reverses exactly what it added. The
      hand-written, gitignored `.claude/settings.local.json` dogfood
      wiring is unaffected and still works alongside it.
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
      three tiles and screenshots them. The six-session version exists
      headless since 2026-09-13 as `app/src-tauri/tests/pipeline.rs`,
      six sessions through the real endpoint into six colours; the
      screenshot walk and the logging decision are still owed.
- [x] Capture the session pid and host at the hook instead of guessing
      later: the shim wraps the payload with `CLAUDE_PID`,
      `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`, and
      `VSCODE_PID` from its own environment (all observed on 2.1.270
      under the VS Code extension; the terminal case is not yet
      observed), and the daemon finds the window by walking that pid's
      ancestors to the first process owning a visible top-level window,
      with the title match limited to that process's windows.
      Superseded by [ADR-032](docs/DECISIONS.md#adr-032) (2026-09-13),
      which reaches the same goal a different way: the shim and its
      payload are unchanged, and Reveal instead takes the `pid`
      `claude agents --json` already reports and classifies the host at
      click time from a Toolhelp32 snapshot of the running process tree
      (`Code.exe`, `WindowsTerminal.exe`, or a plain console), then
      raises through the strategy chosen for that host. It does not
      populate an ADR-023 `host` field; that axis is unaffected.
- [ ] Take liveness from the pid: hold a process handle per session and
      flip to `ended` the moment it exits, and stop greying a live idle
      session at the fifteen-minute mark. Not resolved by ADR-032, which
      only changed how Reveal finds a window for a pid it already has;
      this still needs its own ADR.
- [x] Bring `app/` in line with ADR-028: replace the six-slot tile surface
      and its manual bind picker, command keys, stick, dial, talk and
      send placeholders, detail panel, layer strip, and corner badges
      with the session-list surface (one row per session: colour, glyph,
      name, state word) and a header holding a grey toggle and Quit
      (ADR-030 added Hide grey on top of ADR-028's Move and Quit;
      ADR-031 then removed Move and made the header the drag region).
- [x] Replace the six-slot manual binding with ADR-028's auto-binding:
      bind a session on its first hook event or enumeration hit, into an
      ordered, unbounded list; drop the null slots when loading a legacy
      six-slot `bindings.json`.
- [x] Rerun `claude agents` enumeration every 15 s on its own timer,
      outside the registry lock, and remove a session when it ends or
      when a successful enumeration no longer lists it and it has had no
      hook event for 60 s; a failed enumeration must prune nothing
      (ADR-028).
- [x] Size and place the window per ADR-028: about 360 px wide, height
      following the row count at 64 px per row plus a 52 px header
      (ADR-029 set it at 64 px; ADR-031 shrank it to 52), clamped to the
      monitor work area, with a saved position validated against the
      monitors actually connected at startup.
- [x] Exclude Deckhand's own window from the raise match (ADR-028).
- [ ] Verify the console Reveal path (`AttachConsole` plus
      `GetConsoleWindow`, [ADR-032](docs/DECISIONS.md#adr-032)) against a
      real console-hosted session; so far it has only been exercised
      against a fake process table in unit tests.
- [ ] Fire the VS Code session-tab link
      (`vscode://anthropic.claude-code/open?session=<id>`) once a safe
      cross-window test exists. Blocked on Deckhand's own evidence, per
      [ADR-032](docs/DECISIONS.md#adr-032): a session in VS Code's
      integrated terminal is never tracked by the extension host, and
      which window a multi-window instance routes the URI to has not
      been observed, so firing it today risks a duplicate live session
      rather than revealing the existing one.
- [ ] Windows Terminal tab targeting (raising a specific tab, not only
      the one open Terminal window) is blocked upstream, not by
      Deckhand: `microsoft/terminal#19783` asked for exactly this and
      was closed not planned in January 2026. Revisit if that changes.
- [x] Register the shim in user-level Claude Code settings so sessions in
      every repo report state, not only this one (done on the owner's
      machine with scripts/install-hooks.ps1 on 2026-09-13; the repo-local
      wiring was removed at the same time).

---

## Phase 2: Approve and deny

- [ ] Implement `PreToolUse` hook response with an actual permission
      decision, and the daemon-side hold while that decision is pending.
- [ ] Decide, in its own ADR, what approve and deny look like on the
      session-list surface now that ADR-028 removed the command keys they
      were going to be.
- [ ] Wire the approve control to the held decision, once designed.
- [ ] Wire the deny control to the held decision, once designed.
- [ ] Implement a timeout policy for an unanswered permission decision.
- [ ] Implement the audit trail of approvals and denials.
- [ ] Test the failure mode where the daemon dies while a decision is
      pending.
- [ ] Confirm the amber state can only ever be shown when a decision is
      genuinely pending, with a test for the inverse.
- [ ] Security review of the approve and deny path specifically (see
      `docs/SECURITY_MODEL.md`).

---

## Phase 3: Retired (ADR-028)

Every item this phase used to list, dial stepping and its minus, plus, and
commit targets, the four-way stick, the detail panel, continue and
interrupt keys, the Answer key and its answer targets, the Reveal key,
plan mode and compact in a panel, a layer strip, a settings surface, and
theming, is removed from the plan by
[ADR-028](docs/DECISIONS.md#adr-028) on 2026-09-13. Some of it had
partial code in `app/` from the change just before ADR-028; removing that
code to match the session-list surface is tracked once, in Phase 1 above,
rather than repeated per control here. A control returning to the surface
needs its own ADR and its own line here.

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

## Phase 5: Retired (ADR-028)

The talk control and its MacroVox integration, listed here, are removed
from the plan by [ADR-028](docs/DECISIONS.md#adr-028) on 2026-09-13. A
talk control returning to the surface needs its own ADR and its own line
here.

---

## Phase 6: Beyond Claude Code

- [ ] Choose the second adapter target.
- [ ] Validate Codex as the proposed target using W0 in the
      [OpenAI integration plan](docs/OPENAI_INTEGRATION_PLAN.md): collect
      redacted fixtures and a per-host, per-version evidence matrix.
- [ ] Reconcile the normalized event contract and composite session
      identity with protocol version 0 before implementing W1 and W2.
- [ ] Prove mixed Claude/Codex observation, migration recovery, and
      per-session capability behavior before enabling the second adapter.
- [ ] Validate App Server connection ownership and safe native handback
      before scheduling any OpenAI control capability.
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
