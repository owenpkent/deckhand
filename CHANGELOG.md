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

### Added

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
