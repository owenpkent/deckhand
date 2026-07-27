# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a
Changelog](https://keepachangelog.com/en/1.1.0/), and dates are ISO 8601
(`YYYY-MM-DD`). There are no releases yet: Deckhand is at Phase 0
(specification), so everything so far lives under `[Unreleased]`. Once there
is something to version, releases here will follow [Semantic
Versioning](https://semver.org/); until then, no version number is invented
and no past release is backfilled.

## [Unreleased]

### Fixed

- Documentation sweep across the whole specification, from two independent
  audit passes (consistency and honesty). The one real design contradiction
  found: three documents disagreed on what a dead session process turns a
  tile into. Resolved as: a clean exit is `ended`, a confirmed death without
  a clean exit is a crash and is `error` (acknowledging it then shows
  `ended`), and silence past the liveness deadline stays `unknown`.
  `docs/ARCHITECTURE.md`, `docs/CLAUDE_CODE_ADAPTER.md`, and
  `docs/CONTROL_MAPPING.md` now say the same thing.
- Smaller alignment fixes: the adapter request template now lists all seven
  capabilities in the protocol's order; the PR sync checklist covers all
  eight authoritative documents; the source-of-truth map gained rows for
  `docs/ARCHITECTURE.md` and `docs/ACCESSIBILITY.md`; `TODO.md` no longer
  lists as open three decisions that ADR-007 and the adapter doc had
  already made; state tables use one row order and one name per concept
  (`needs_input`, pinned mode); phase labels for the two de-risking spikes
  agree (Phase 0, gating Phase 1); auto-approval rules are unscheduled
  rather than promised for Phase 3; the executive summary and README now
  hedge unbuilt behaviour and carry the non-affiliation note.

### Added

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
  observation and approve and deny authority through hooks, but cannot
  inject a prompt into a running interactive session because Claude Code
  exposes no supported mechanism for that; hosted mode starts sessions
  through the Claude Agent SDK and gets full control, including send, at
  the cost of the normal terminal UI. This split is documented as the
  central architectural fact of the project, because it determines what
  every other document has to account for.
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
