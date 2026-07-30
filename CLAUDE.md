# CLAUDE.md: Deckhand AI onboarding

Authoritative context for AI assistants working in this repo. Read this first,
then [docs/WORKFLOW.md](docs/WORKFLOW.md) before editing anything.

## Project overview

- **Name:** Deckhand (`owenpkent/deckhand`)
- **What:** A software reimplementation of the Codex Micro macropad as an
  always-on-top, mouse-only control surface for Claude Code sessions. Six
  status tiles, approve and deny via the `PreToolUse` hook, dial, stick,
  talk, layers.
- **Status:** Phase 0, specification only. **There is no application code.**
  Do not "fix" that by scaffolding code unprompted; Phase 1 is gated on the
  two pre-Phase-1 spikes tracked in [TODO.md](TODO.md) and
  [docs/DECISIONS.md](docs/DECISIONS.md#adr-009).
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
come from the Codex Micro and are frozen by
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
| [docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md) | 291 | What every control does |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 401 | Daemon, shim, surface, state machine |
| [docs/ADAPTER_PROTOCOL.md](docs/ADAPTER_PROTOCOL.md) | 310 | Daemon to runtime contract |
| [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) | 495 | Reference adapter; partial stamp |
| [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) | 273 | Approval path; fails to `ask` |
| [docs/UI_SPEC.md](docs/UI_SPEC.md) | 294 | Visual and interaction contract |
| [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md) | 182 | The rules that win every conflict |
| [docs/DECISIONS.md](docs/DECISIONS.md) | 585 | ADRs; append only |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | 136 | Map and change-propagation table |
| [ROADMAP.md](ROADMAP.md) / [TODO.md](TODO.md) | 182 / 225 | Phases and open work |

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
- **Next ADR: 023.** ADRs are append-only, contiguous, and anchored; a
  decision is changed by adding a superseding entry, never by editing one.
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
   wholly unverified either.
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

Finish Phase 0: spec complete and internally consistent. The adapter now
carries a partial verification stamp against Claude Code 2.1.220, and ADRs
013 to 022 record the changes that came out of it. The two pre-Phase-1
spikes are still the next real work: prove the no-focus-steal window in
Tauri on Windows (untouched), and validate hook payloads against a live
Claude Code install (partially advanced, and still open, because reading a
string in documentation or a binary is not observing a payload).
