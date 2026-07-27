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

## Key files

| File | Purpose |
| --- | --- |
| [docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md) | Source of truth for what every control does |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Daemon, shim, surface, session state machine |
| [docs/ADAPTER_PROTOCOL.md](docs/ADAPTER_PROTOCOL.md) | Contract between daemon and agent runtimes |
| [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) | Reference adapter; stability-annotated; unverified |
| [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) | Rules for the approval path; fail to `ask` |
| [docs/UI_SPEC.md](docs/UI_SPEC.md) | Visual and interaction contract |
| [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md) | The rules that win every conflict |
| [docs/DECISIONS.md](docs/DECISIONS.md) | ADRs; add entries, never rewrite them |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | Source-of-truth map and change-propagation table |
| [ROADMAP.md](ROADMAP.md) / [TODO.md](TODO.md) | Phases and open work; Constellation scrapes TODO checkboxes |

## Conventions

- **Commits:** Conventional Commits, lowercase subject, no trailing period:
  `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, scoped like `docs(adapter):`
  when useful. One logical change per commit.
- **Never add AI attribution to commits.** No `Co-Authored-By: Claude`, no
  "Generated with" lines. Firm rule.
- **Branches:** `feature/...`, `fix/...`, `docs/...`. Work lands on `main`.
- **Docs style:** plain and honest, no hype, hedge what is unproven. Wrap at
  roughly 80 columns. **Never use em dashes or en dashes**; use commas,
  colons, parentheses, or full stops. CI enforces this.
- **Status claims:** every design doc carries a status line (`proposed`,
  `accepted`, `verified against version X`). Never upgrade a status without
  the thing that justifies it.
- **AI scratch space:** `_scratch/` (gitignored). Never commit temp files.
- **Push discipline:** only at coherent boundaries: docs consistent, links
  resolving, CI green.

## Things to watch out for

1. The six status colours come from the Codex Micro and their meanings are
   frozen (ADR-008). `unknown` is the only Deckhand addition. Do not invent
   states or repurpose colours.
2. The approval path must fail to `ask`, never to `allow` (ADR-006). Any
   edit that touches it must keep every non-human exit path safe, and update
   [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) in the same change.
3. Claude Code integration facts in
   [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) are **unverified
   against a live install**. If you verify one, update the stamp; if you cite
   one elsewhere, keep the hedge.
4. Interaction rules in [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md) are
   requirements, not guidance: no required holds, drags, double-clicks,
   hovers, or keyboard. 44 px minimum targets.
5. When you change any control, capability, state, or default, walk the
   change-propagation table in [docs/WORKFLOW.md](docs/WORKFLOW.md) and update
   every file in the row. The PR template's sync checklist exists to force
   this.

## Quick commands

```powershell
git status
git log --oneline -10
rg -n "something" docs/
# Style gate (same check CI runs): find any em/en dashes in tracked markdown
rg -n "\x{2014}|\x{2013}" --glob "*.md" --glob "!CODE_OF_CONDUCT.md" --glob "!CONSTELLATION_INTEGRATION_GUIDE.md"
```

## Constellation

This repo is read by [Constellation](https://github.com/owenpkent/constellation),
the owner's cross-project dashboard. Keep compatible:

1. `README.md` keeps a `## Status` section naming the current phase.
2. `TODO.md` uses `- [ ]` / `- [x]` checkboxes; Constellation scrapes them.
3. This file stays current when focus shifts.

## Current focus

Finish Phase 0: spec complete and internally consistent. Next real work is
the two pre-Phase-1 spikes: prove the no-focus-steal window in Tauri on Windows,
and validate hook payloads against a live Claude Code install.
