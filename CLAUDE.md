# CLAUDE.md: Deckhand AI onboarding

Authoritative context for AI assistants working in this repo. Read this first,
then [docs/WORKFLOW.md](docs/WORKFLOW.md) before editing anything.

## Project overview

- **Name:** Deckhand (`owenpkent/deckhand`)
- **What:** A software reimplementation of a hardware macropad as an
  always-on-top, mouse-only control surface for Claude Code sessions. An
  ordered list of session rows (colour, glyph, name, state word), click
  to select and raise, plus a Hide grey switch, a gear button, and Quit
  in the header, the whole bar itself the drag region (ADR-030, ADR-031).
  The gear opens an in-bar settings panel in place of the session list
  (ADR-033): always on top, start with Windows, reset position, and a
  hooks-installed status with a Repair action; Hide grey stays a header
  control throughout, never moving into the panel. Underneath the
  surface, ADR-037 gives the daemon a process lifecycle: single
  instance by a named mutex, and a same-binary watchdog that restarts
  it after a crash; Start with Windows itself is unchanged. ADR-038
  hides a session row the VS Code extension has kept alive as a stale
  duplicate of a newer one in the same window, and gives every row one
  label rule instead of two. Move, a
  click-to-place alternative to dragging, was removed by
  ADR-031: the window is now repositioned by dragging only, an
  owner-approved exception to the no-required-drag rule in
  [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md), not a compliant
  default. Approve and deny via the `PreToolUse` hook are Phase 2 and
  not currently planned on the surface (ADR-028); the dial, stick,
  talk, and layers this overview used to list here are removed by the
  same decision.
- **Status:** Phase 1, observation only, started 2026-08-02. The code
  lives in `app/` (daemon plus surface, one Tauri application) and
  `shim/`; `spikes/` is frozen Phase 0 evidence. Nothing has write
  authority: approve and deny are Phase 2 and are not to be wired early.
  Open Phase 1 work is tracked in [TODO.md](TODO.md).
- **Stack (decided, not built):** Tauri v2, Rust daemon, TypeScript surface.
  See [docs/DECISIONS.md](docs/DECISIONS.md#adr-002).

## About the owner

Owen (`owenpkent`) is a wheelchair user with muscular dystrophy. Typing is
expensive; the pointer is cheap. Working agreement:

- Be proactive. Decide and act on small things; do not ask for confirmation
  on routine steps.
- When a real decision is needed, offer lettered options (A/B/C) so the
  answer can be one character.
- Keep replies terse. Long explanations cost more than they give.
- Shell examples in docs are PowerShell.
- Mouse-only operation is the project's premise. Any suggestion that assumes
  keyboard use is wrong by default.

## Frozen constants

Inlined so no file read is needed to check them. Six colours and meanings
come from the hardware macropad and are frozen by
[ADR-008](docs/DECISIONS.md#adr-008); `unknown` is the only Deckhand
addition. Do not invent states or repurpose colours.

| State | Colour | Meaning |
| --- | --- | --- |
| `idle` | white | Bound, nothing running |
| `thinking` | blue | Turn or tool call in flight |
| `complete` | green | Finished and unread; clears on tile selection |
| `needs_input` | amber | Waiting on a human |
| `error` | red | The turn failed |
| `ended` / `unbound` | off | Session ended, or no session on this tile |
| `unknown` | grey | Observation degraded; never a guess |

Colour is never the only channel: every state carries a glyph and a label.
Minimum hit target is **44 px**, and that is a floor, not a target.

## Key files

[docs/WORKFLOW.md](docs/WORKFLOW.md) section 1 is the authoritative
source-of-truth map. This table is a reading budget, not a second map.

| File | Lines | Purpose |
| --- | --- | --- |
| [docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md) | 296 | What every control does |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 696 | Daemon, shim, surface, state machine |
| [docs/ADAPTER_PROTOCOL.md](docs/ADAPTER_PROTOCOL.md) | 362 | Daemon to runtime contract |
| [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) | 780 | Reference adapter; partial stamp |
| [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) | 398 | Approval path; fails to `ask` |
| [docs/UI_SPEC.md](docs/UI_SPEC.md) | 285 | Visual and interaction contract |
| [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md) | 231 | The rules that win every conflict |
| [docs/DECISIONS.md](docs/DECISIONS.md) | 2163 | ADRs; append only |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | 158 | Map and change-propagation table |
| [docs/UPSTREAM_ASKS.md](docs/UPSTREAM_ASKS.md) | 311 | What we need from the runtimes |
| [ROADMAP.md](ROADMAP.md) / [TODO.md](TODO.md) | 185 / 472 | Phases and open work |

**Do not read `CONSTELLATION_INTEGRATION_GUIDE.md`.** It is 380 lines of
generic vendor boilerplate sitting at the repo root, where it matches
searches for TODO, status, commit, and PowerShell and answers none of them.
The only binding parts are the three rules in the Constellation section
below. Skip it in searches.

## Conventions

- **Commits:** Conventional Commits, lowercase subject, no trailing period:
  `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, scoped like `docs(adapter):`
  when useful. One logical change per commit.
- **Never add AI attribution to commits.** No `Co-Authored-By: Claude`, no
  "Generated with" lines. Firm rule, enforced by the local `PreToolUse`
  gate in `.claude/hooks/style-gate.js`.
- **Branches:** solo work commits direct to `main`. Branches
  (`feature/...`, `fix/...`, `docs/...`) and the PR template are for larger
  or riskier changes and for outside contributors. This resolves the
  apparent contradiction with [CONTRIBUTING.md](CONTRIBUTING.md), which is
  written for contributors, not for the owner.
- **Docs style:** plain and honest, no hype, hedge what is unproven. Wrap at
  roughly 80 columns. **Never use em dashes or en dashes**; use commas,
  colons, parentheses, or full stops.
- **The style rules live in one place:** `scripts/check-docs.ps1`. It is
  what CI runs and what `/docs-gate` runs. Run
  `powershell -NoProfile -File scripts/check-docs.ps1 -All` before pushing
  (CI calls the same script with `pwsh`; this machine has no `pwsh`) rather
  than reciting the rules by hand.
- **Status claims:** every design doc carries a status line (`proposed`,
  `accepted`, `verified against version X`). Never upgrade a status without
  the thing that justifies it.
- **Next ADR: 039.** ADRs are append-only, contiguous, and anchored; a
  decision is changed by adding a superseding entry, never by editing one.
- **When to write an ADR:** only for big decisions: the security model,
  the approval path, what Deckhand may do, adapter capabilities, new
  dependencies, the stack, or a frozen constant. UI layout, styling,
  control placement, and wording changes get a CHANGELOG line and the
  spec files from the WORKFLOW.md table, not an ADR. The owner does not
  manage ADRs; this is the assistant's call.
- **AI scratch space:** `_scratch/` (gitignored). Never commit temp files.
- **Push discipline:** only at coherent boundaries: docs consistent, links
  resolving, CI green.

## Things to watch out for

1. The approval path must fail to `ask`, never to `allow` (ADR-006). Any
   edit that touches it must keep every non-human exit path safe, and update
   [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) in the same change.
2. Claude Code integration facts in
   [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) carry a
   **partial** verification stamp against 2.1.220. Four things are observed;
   hook names, payload fields, and the decision vocabulary are `documented`
   at best. Keep the hedge when citing them, and do not claim the file is
   wholly unverified either. The scan's `status` key is a separate,
   narrower observation, against 2.1.270, not against 2.1.220
   ([ADR-035](docs/DECISIONS.md#adr-035)). The session-record fields that
   `claude agents --json` drops are a third, against 2.1.273 on
   2026-09-16. The reason `Notification` has never fired here, a six
   second inactivity delay, is `documented` from a maintainer comment and
   is not observed; see
   [docs/UPSTREAM_ASKS.md](docs/UPSTREAM_ASKS.md) section 4.3.
3. Interaction rules in [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md) are
   requirements, not guidance: no required holds, drags, double-clicks,
   hovers, or keyboard. 44 px minimum targets.
4. When you change any control, capability, state, timing, hook event, or
   default, walk the change-propagation table in
   [docs/WORKFLOW.md](docs/WORKFLOW.md) and update every file in the row.
   The `sync-check` skill does that walk; use it instead of re-deriving the
   table by hand.

## Constellation

This repo is read by [Constellation](https://github.com/owenpkent/constellation),
the owner's cross-project dashboard. Keep compatible:

1. `README.md` keeps a `## Status` section naming the current phase.
2. `TODO.md` uses `- [ ]` / `- [x]` checkboxes; Constellation scrapes them.
3. This file stays current when focus shifts.

## Current focus

Phase 1: make observation trustworthy. The skeleton builds, passes its
state machine tests, and paints real tiles from synthetic events through
the real shim; `scripts/build-app.ps1` builds it, and gitignored
`.claude/settings.local.json` wires this repo's sessions into the shim
for dogfooding. The open Phase 1 work is in [TODO.md](TODO.md):
installable hook registration and the six-session colour test.
ADR-023 added the host axis, ADR-024
corrected the enumeration to bindings-not-state, and ADR-025 recorded
the window spike pass that Phase 1's window builds on. ADR-027 folded
the raise into the tile click: selecting a session brings its window
forward. ADR-028 then cut the surface down to that session list plus
Move and Quit, replacing the six-slot binding with an auto-binding,
unbounded list; ADR-030 added a third header control, Hide grey, that
filters `unknown` rows out of the list and shows how many it hid on the
button itself. ADR-031 then removed Move, made the whole header a drag
region, relabelled and restyled the grey toggle, and dropped the dashed
outline unknown and ended rows used to share. ADR-029 had already
restyled the list (64 px two-line rows, header counts, a bundled
typeface), and ADR-032 made Reveal host-aware (VS Code, Windows
Terminal, console). ADR-033 then added a gear-triggered settings panel
beside the header's existing grey toggle, holding four new settings
(always on top, start with Windows, reset position, and a hooks
status with Repair); the grey toggle itself stayed in the header.
ADR-034 redrew the header and that panel the next day: icon buttons
for the gear and Quit, toggle switches (the header's grey toggle
included, relabelled "Hide grey" / "N hidden"), a combined Hooks and
Repair row, two titled panel sections, rounded rows with a left
accent bar, a shared inset for rows and panel cards, and a
window-sizing fix for the panel's own scrollbar. ADR-035, the day
after that, gave the daemon a second liveness channel: a process
handle held per session, polled on the existing two-second tick, so
exit without a `SessionEnd` moves a session straight to `ended`
instead of leaving a red or grey row behind. Where a hook has not yet
coloured a session, the periodic scan now does, from `status` on the
installed 2.1.270; `T_unknown` narrows so a session with a live
handle stops greying from silence alone. ADR-036, the same day,
gave the scan one more job: after two consecutive scans contradict a
hook-set colour with no hook between them, about thirty seconds, the
scan breaks the tie instead of waiting on `T_unknown`. The planned
transcript fallback is retired with it, not built; the scan's
`status` already answers what it would have, from a documented
command. ADR-037, the same day again, closed the daemon's last open
lifecycle question: a named mutex makes the app single instance, and
a same-binary watchdog, spawned by every launch, relaunches it after
a crash and rate-limits itself with a local, append-only ledger.
Start with Windows stays exactly as ADR-033 left it. ADR-038, the
same day again, hid a duplicate the VS Code extension caused: an
idle, complete, or unknown session is now left off the list while a
newer bound session shares its window, parent process, and folder,
and reappears on its own once it needs the owner or the newer one
ends. The same decision gave every row one label rule, the scan's
`name` when there is one and the directory name otherwise, tracked by
a `derived` flag in `bindings.json` rather than guessed from strings.
`app/` implements all of these.

Of the two pre-Phase-1 spikes, the window spike is done: on 2026-08-02
`spikes/tauri-focus/` proved the no-focus-steal window in Tauri on
Windows, recorded as ADR-025. The payload spike advanced twice the same
day: a `PreToolUse` deny was honoured from a live session, and the
capture hook `.claude/hooks/payload-capture.js`, registered for all
twelve documented event names, has now seen ten of them fire with full
payloads (ADR-026 and after). Only `Notification` and `StopFailure`
remain unobserved, and the live corrections that came out of validation
are in the ADR. Phase 1 has started and the board has painted real
sessions.
