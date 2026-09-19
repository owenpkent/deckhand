# Ideas

This is an unfiltered scratchpad. Nothing here is committed to. Some of it
is good, some of it is not, and none of it has been sorted properly yet.

**Promotion path:** idea here, then to `TODO.md` once it is scoped enough to
be an actionable item, then to `ROADMAP.md` once it is committed to a phase.
Most ideas on this page will never make that trip, and that is fine. That is
what this file is for.

Ideas marked **(speculative)** are further out than the rest: bigger leaps,
open questions about whether they even make sense, or dependent on something
outside this project. Everything else here is merely unscheduled, not
vetted.

---

## Surface

- A tile shows a live diff count for its session (lines changed, files
  touched) instead of only a status colour.
- A tile shows the name of the tool currently running, not just that
  something is running.
- A "context remaining" ring drawn around each tile, sourced from statusline
  JSON if Claude Code exposes enough to compute it. **(speculative, depends
  on what statusline actually exposes)**
- A stacked bar somewhere in the detail panel showing where a session's
  tokens went: input, output, cache, tool results.
- Sharing a layout (tile bindings, layer strip contents, theme) as a single
  file another Deckhand install can import.

## Status

- A "why did it ask" panel: when a tile goes amber, show the actual tool
  input that triggered the permission request, not just the tool name.
- A "degraded" status distinct from both idle and error, for when the
  daemon can see a session exists but has stopped receiving hook events for
  it. Not the same as red, not the same as white. **(speculative)**

## Actions

- A rule engine: auto-approve read-only tool calls, always ask for anything
  that writes. Would need to live entirely on the local machine and be very
  legible about what it is doing, given `docs/SECURITY_MODEL.md` treats the
  approve button as a security surface.
- Auto-bind tiles to the six most recently active Claude Code sessions,
  instead of requiring manual binding every time.
- Export a session's approval and denial history as an audit log: plain
  text or CSV, something greppable.
- A prompt tray: six customisable keys, mirroring the six the hardware
  actually shipped with, against Deckhand's six hardcoded command-key
  actions, which is an undeclared divergence worth naming even if the keys
  stay hardcoded. Gated entirely on a send channel that does not exist yet
  in attached mode, so held here, not promoted. **(speculative)**

## Ecosystem

- An adapter for the sibling Nimbus joystick project, so the surface can be
  driven without a mouse at all. Worth sitting with the irony deliberately:
  Deckhand's whole premise is mouse-only operation for people for whom other
  input is harder, so a joystick adapter would need to be an additional
  option, never a replacement that narrows who can use the surface.
  **(speculative)**
- Something with Octavium (MIDI), possibly using a MIDI controller's pads or
  knobs as a physical proxy for the dial and command keys. **(speculative,
  no concrete shape yet)**
- Tighter MacroVox integration beyond push-to-talk, for example a status
  change read aloud on amber.
- The sibling Anchor project (`..nchor`) as the engine behind hosted
  mode. Anchor is a Python daemon that starts and owns Claude Agent SDK
  sessions, gates every consequential tool call through an approval broker
  that resolves to "the tool does not run" on timeout and never to allow,
  and writes a hash-chained audit log. Its front-ends implement a
  three-method channel protocol (post text, request approval, close
  approval), which is close to what a hosted tile needs, so Deckhand would
  be a local channel next to Anchor's Slack one, talking over the daemon's
  existing local transport. It does nothing for attached mode: it has no
  knowledge of Claude Code's hooks or session ids. Its broker and policy
  code are also worth reading before the Phase 2 ADR, although the
  plumbing differs (Anchor answers an in-process SDK callback, Deckhand
  answers a `PreToolUse` hook subprocess) and so do the languages, so it
  is a design to copy, not code to import. Anchor has not yet run against
  a live model, so nothing should be wired until that and Phase 1 here are
  both done. The matching note on the Anchor side is its
  `docs/DECKHAND.md`. **(speculative, depends on Anchor reaching its first
  live milestone, and on deciding whether a Rust app should depend on a
  Python daemon at all)**

## Wild

- A wall-mounted tablet running Deckhand as a physical status board for a
  room, not just a desktop overlay. **(speculative, arguably a different
  product, but the tile model would mostly carry over)**
- A shared, multi-viewer mode where more than one person can see, though not
  necessarily act on, the same set of tiles. Raises real questions under
  `docs/SECURITY_MODEL.md` about who gets to approve what. **(speculative)**
- Historical replay: scrub back through a session's status transitions
  after the fact, to see what an agent actually did overnight.
  **(speculative)**
