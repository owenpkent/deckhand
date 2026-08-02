# UI specification

Status: **proposed**. This is the visual and interaction contract for the
surface. Behaviour semantics live in [CONTROL_MAPPING.md](CONTROL_MAPPING.md);
constraints on interaction live in [ACCESSIBILITY.md](ACCESSIBILITY.md) and
win any conflict with this file.

Illustrative mockups of this spec live in [`assets/`](../assets/) and are
embedded in the README. They are drawings of the spec, not screenshots; where
a mockup and this file disagree, this file wins.

## The surface

A frameless, always-on-top panel that never takes keyboard focus. Two
orientations:

```
 Horizontal (default, docked bottom or top)

 ┌──────────────────────────────────────────────────────────────────────┐
 │ [T1][T2][T3][T4][T5][T6] │ [✔][✘][?][■][▶][◎] │ ◄stick► │ dial │ 🎙 ➤ │
 └──────────────────────────────────────────────────────────────────────┘

 Vertical (docked left or right)         Detail panel (expands on demand)

 ┌──────┐                                ┌──────────────────────────────┐
 │ [T1] │                                │ tile 3 · undertow · Opus     │
 │ [T2] │                                │ WAITING ON YOU               │
 │ [T3] │                                │ Bash: cmake --build ...      │
 │ ...  │                                │ [ Approve ]      [ Deny ]    │
 │ keys │                                │ context ▓▓▓░ 62%   $0.41     │
 │ dial │                                │ [ Reveal ]       [ Unbind ]  │
 └──────┘                                └──────────────────────────────┘
```

- Docks to any screen edge with a click-to-place move mode (no drag required;
  drag also works).
- Collapsible to a tile-strip-only sliver; the tiles are the part that must
  survive collapse, because glancing is the product.
- Idle dim after 3 minutes without pointer interaction or state change,
  matching the device's lighting timeout. Any state change wakes it. Dim, not
  hide: a status board that hides is not a status board.
- Remembers position, orientation, scale, and collapse state per monitor
  arrangement.

## Tile anatomy

```
 ┌────────────┐
 │ ◠ spinner  │   status ring, 3 px, state colour
 │  undertow  │   slot 1: repo dir name, user-renamable
 │  Bash ▸    │   slot 2: current tool / last event
 │  ▓▓▓░ 4:12 │   slot 3: elapsed, slot 4: context bar
 │ auto    ×2 │   corner badges: permission mode, live children
 └────────────┘
```

- Default tile: 96 by 96 dip at 100% scale. Never below 44 by 44, per the
  accessibility floor.
- Status is triple-coded on every tile: ring colour, glyph, and the label text
  in the detail panel. See the state table in
  [ACCESSIBILITY.md](ACCESSIBILITY.md#status-without-colour).
- Selected tile: thicker ring plus a small chevron notch; pulse optional.
- Unbound tile: dashed outline, a plus glyph, single click opens the bind
  picker.
- Green (complete) clears to white when the tile is *selected*, nothing else.

### Tile budget

A tile is 96 by 96 dip and has to survive 300% scale, so its contents are a
fixed budget: **four content slots plus two corner badges**. Nothing new lands
on a tile without displacing something in this table.

| Rank | Element | Content | Value per pixel |
| --- | --- | --- | --- |
| 1 | Ring, fill, glyph | The state, triple-coded | Highest: it is the whole reason the board exists |
| 2 | Slot 1 | Session label (repo dir name) | Which session this is; useless without it |
| 3 | Slot 2 | Current tool or last event | What it is doing, the second question always asked |
| 4 | Slot 3 | Elapsed-in-operation while an operation is open, elapsed-in-state otherwise | Slow or stuck, answerable from the panel instead |
| 5 | Slot 4 | Context bar | Rarely acted on within a glance |

Slot 3 shows elapsed-in-operation whenever an operation is open on the
session, because "this tool call has been running four minutes" is the number
a person can act on, and falls back to elapsed-in-state when none is. The rule
is in [ARCHITECTURE.md](ARCHITECTURE.md#liveness-by-open-operation).

Rank is value per pixel and it doubles as the collapse order run backwards. At
200% scale and above, slot 4 collapses first and slot 3 next; ranks 1 to 3
never collapse, and a layout that cannot keep them is the wrong layout. Every
collapsed slot is still readable in words in the detail panel.

### Corner badges

Two, and they are **never hit targets**. A badge is read, never clicked, so the
44 dip floor does not bind on it and it cannot become a control by accident.
Everything a badge says also appears in words in the detail panel, because a
badge is a glance, not a route.

- **Top-right: permission mode.** Text, never a colour. All six colours are
  already spent on state (ADR-008) and a mode is not a state. The values are
  listed in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types), plus `unknown`
  for a payload that does not carry one or carries one the adapter does not
  recognise, and `unknown` renders as the word rather than as a blank.
  Six modes exist (`acceptEdits`, `auto`, `bypassPermissions`, `manual`,
  `dontAsk`, `plan`, observed against Claude Code 2.1.220), and `manual`,
  `auto`, and `dontAsk` are the three that change what a
  Deckhand `ask` falls through to; see
  [SECURITY_MODEL.md](SECURITY_MODEL.md#1-fail-closed-in-the-correct-direction).
- **Bottom-right: live children.** The count of open entries in the session's
  child ledger, shown only when that count is above zero.

### The child ledger and COMPLETE

**COMPLETE is unreachable while the child ledger is non-empty.** Around 10.5%
of turns ended with children still running in the one corpus that has been
measured, 240 sessions on the author's machine, which is one user's habits and
not a general finding. On that machine a tile that turns green on the parent's
stop is green while work continues, which falsifies the one promise the board
makes. A session whose parent turn has finished but whose
ledger still has entries stays in thinking, with the child count visible, and
goes green when the ledger empties.

The ledger holds `kind: "subagent"` entries only, opened by `SubagentStart` and
closed by `SubagentStop`. Background Bash tasks emit no subagent hook at all
and are invisible to it, so the count is a floor, not a total, and this file
says so rather than implying a complete picture.

There is no per-child list, no per-child approval target, and no subagent
layer. The count is a count.

### State rendering

| State | Ring | Fill | Glyph | Motion (reduced-motion variant) |
| --- | --- | --- | --- | --- |
| Idle | White | Neutral | Open circle | None |
| Thinking | Blue | Neutral | Arc spinner | Static arc badge |
| Needs input | Amber | Tinted 20% | Hand | Slow breathe (static) |
| Complete | Green | Tinted 10% | Check | One pulse on entry (none) |
| Error | Red | Tinted 20% | Cross | None |
| Unknown | Grey | Hatched | Question | None |
| Ended / unbound | None | None | Dashed outline / plus | None |

Glyphs are drawn, not emoji, so they render identically across platforms and
respect the theme.

The six colours and their meanings are frozen (ADR-008), so this table is not
where new states arrive. Amber's two kinds, `permission` and `question`, share
this row exactly: same ring, same fill, same glyph, same label. The kind
changes which controls are enabled and what the detail panel shows, never how
the tile looks. See
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#amber-carries-a-kind).

## Command keys

Two rows of three, adjacent to the tile strip. Each key: glyph plus a short
label, never glyph-only, minimum 44 dip, with the mandated dead gap between
Approve and Deny.

| Key | Enabled when | Style |
| --- | --- | --- |
| Approve | Selected tile amber, `kind: permission` | Constructive |
| Deny | Same | Destructive |
| Answer | Selected tile amber, `kind: question` | Neutral, opens the answer targets |
| Interrupt | Selected tile thinking | Destructive |
| Continue | Selected tile complete and a send channel exists | Neutral, normally disabled in attached mode |
| Reveal | Selected tile bound to a locatable host | Neutral |

Plan mode and Compact are not keys. They live in the detail panel; the
measurement that demoted them is in
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#command-keys).

Disabled keys stay visible, and **clicking one is never a no-op: it reveals
why**. The reason renders in the detail panel in words ("No pending request",
"This is a question, not a permission", "Permission mode is auto", "Not
supported in attached mode"), and if the panel is collapsed the click expands
it first. Capabilities the adapter marks `synthetic` render with a corner tick,
and clicking the key reveals what synthetic means for that control before it is
used.

No reason lives in a tooltip. The surface never takes keyboard focus, so a
dwell or eye-tracker user has no route to a hover at all, and a hover-only
reveal is forbidden outright by
[ACCESSIBILITY.md](ACCESSIBILITY.md#forbidden-interactions). A button that
would do nothing silently is the one forbidden state.

### Answer targets

When the selected tile is amber with `kind: question`, the detail panel renders
the question text and **one target per option**.

- The target shows the **full option label**, never a bare letter, number, or
  index. "A" is not something a person can check before clicking it.
- 44 dip floor, plus the half-target dead gap between adjacent targets, the
  same rule Approve and Deny get. Measured questions carry 2 to 4 options, so
  the gap is affordable.
- Options wrap; they are never truncated to fit one line. A truncated option is
  a decision made without the text of it.
- Multi-select questions render the same targets plus one confirm target.
  Nothing is submitted on the first click.
- If the adapter does not declare `answer_question`, the targets render
  disabled and clicking one reveals that this session has no answer channel.
  The question is still shown: hiding it would make the tile amber for no
  visible reason.

## Dial

An arc with three click targets: minus, plus, and centre commit. The arc
sweep is a position indicator; dragging it works but is never required.

- Composer mode: minus and plus step across options (model, effort, permission
  mode), centre commits and advances.
- Pinned mode: the dial is bound to one option; default pin is the model.
- Current option name and value render inside the arc in full words, no
  abbreviation guessing.
- The steppers never reach `dontAsk` or `bypassPermissions`. Those two values
  are outside the stepper's range entirely, so neither is one click away from
  a plus and a minus.
- In attached mode the dial is a readout: the commit target is disabled, and
  clicking it reveals that no observed interface sets these values on a running
  session.

## Stick

A four-way pad, rendered as a diamond of four 44 dip buttons: up (scroll the
detail panel up), down (scroll it down), right (expand or collapse the panel),
left (return to the previously selected tile). Discrete clicks, no analog
anything, no held repeat by default (hold repeat is an opt-in). Up and down
scroll by a fixed step, so a long pending tool input is readable without a
wheel or a drag. The pad does not step tiles: a tile is already one click away
on the strip.

## Talk and send

- Talk: click to start, click to stop, per the accessibility rules. While
  recording, a sea-green trace runs around the surface border, the device's
  travelling light translated to a bezel. Reduced motion: steady sea-green
  border. Hold-to-talk is opt-in.
- Send: adjacent to talk, enabled only when the selected session's adapter
  declares `send_prompt` and there is composed content to send. In attached
  mode this is normally disabled; clicking it reveals the honest reason in the
  detail panel, expanding the panel if it is collapsed.

## Layer strip

A thin row of dots or short labels, one per profile, single click to switch.
Layer 1 is Claude Code. The strip is the one place the 44 dip floor genuinely
binds; if labels do not fit, the strip renders as dots and one click expands
the whole strip into labelled targets. Never a tooltip: a dot that only names
itself on hover is unreachable for a dwell user.

## Detail panel

Expands from the surface (stick-right or click the selected tile's chevron).
Contents in order: session identity line (tile number, label, model, permission
mode, attached or hosted), the state in words, the current or pending item
(tool name and input for amber `permission`, the question and its answer
targets for amber `question`, last result line for green), Approve and Deny
when amber `permission`, context bar with percentage, cost figure, then Reveal,
Unbind, plan mode, compact, and per-session settings. The panel is also where
every double-click accelerator has its single-click equivalent (Reveal, most
importantly).

It is also where reveals land. Clicking a disabled control puts the reason
here, in words, and expands the panel first if it was collapsed. That is the
only mechanism: nothing on this surface explains itself by hover.

Tool input display: monospace, wrapped, truncated past 12 lines with an expand
control. The stick's up and down scroll it. Truncation never disables the
decision buttons.

## Theming

- Three built-in themes: dark (default), light, high-contrast. Theme tokens
  (colour roles, radii, gaps) live in one TypeScript module; nothing hardcodes
  a hex value outside it.
- The six state colours are semantic tokens shared by every theme; the
  high-contrast theme may shift their luminance, never their hue mapping.
- Scale: 100% to 300%, one slider, everything moves together.
- Reduced motion: honours the OS setting and has its own override switch.

## Sound

Off by default. Optional short cues for amber and red only, distinct shapes,
each individually toggleable. Green and blue never make noise; a surface for
six parallel agents that chirps on every completion trains you to mute it.

## Empty and error states of the surface itself

- No daemon: the surface shows one full-width card saying the daemon is not
  reachable, with a start action. Never six grey tiles pretending.
- No sessions bound: tiles show the bind affordance and a one-line hint
  pointing at the picker. No tutorial overlay, no tour.
- Adapter degraded (observing but not acting): a thin warning bar names the
  lost capability rather than letting buttons fail on click.
