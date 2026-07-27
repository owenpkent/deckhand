# UI specification

Status: **proposed**. This is the visual and interaction contract for the
surface. Behaviour semantics live in [CONTROL_MAPPING.md](CONTROL_MAPPING.md);
constraints on interaction live in [ACCESSIBILITY.md](ACCESSIBILITY.md) and
win any conflict with this file.

## The surface

A frameless, always-on-top panel that never takes keyboard focus. Two
orientations:

```
 Horizontal (default, docked bottom or top)

 ┌──────────────────────────────────────────────────────────────────────┐
 │ [T1][T2][T3][T4][T5][T6] │ [✔][✘][▶][■][P][C] │ ◄stick► │ dial │ 🎙 ➤ │
 └──────────────────────────────────────────────────────────────────────┘

 Vertical (docked left or right)         Detail panel (expands on demand)

 ┌──────┐                                ┌──────────────────────────────┐
 │ [T1] │                                │ tile 3 · undertow · Opus     │
 │ [T2] │                                │ WAITING ON YOU               │
 │ [T3] │                                │ Bash: cmake --build ...      │
 │ ...  │                                │ [ Approve ]      [ Deny ]    │
 │ keys │                                │ context ▓▓▓░ 62%   $0.41     │
 │ dial │                                │ [ Raise window ] [ Unbind ]  │
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
 │  undertow  │   label: repo dir name, user-renamable
 │  Bash ▸    │   subtitle: current tool / last event
 │  ▓▓▓░ 4:12 │   context bar and elapsed-in-state
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

### State rendering

| State | Ring | Fill | Glyph | Motion (reduced-motion variant) |
| --- | --- | --- | --- | --- |
| Idle | White | Neutral | Open circle | None |
| Thinking | Blue | Neutral | Arc spinner (static arc badge) |
| Complete | Green | Tinted 10% | Check | One pulse on entry (none) |
| Needs input | Amber | Tinted 20% | Hand | Slow breathe (static) |
| Error | Red | Tinted 20% | Cross | None |
| Unknown | Grey | Hatched | Question | None |
| Ended / unbound | None | None | Dashed outline / plus | None |

Glyphs are drawn, not emoji, so they render identically across platforms and
respect the theme.

## Command keys

Two rows of three, adjacent to the tile strip. Each key: glyph plus a short
label, never glyph-only, minimum 44 dip, with the mandated dead gap between
Approve and Deny.

| Key | Enabled when | Style |
| --- | --- | --- |
| Approve | Selected tile amber with a pending request | Constructive |
| Deny | Same | Destructive |
| Continue | Selected tile complete or idle | Neutral |
| Interrupt | Selected tile thinking | Destructive |
| Plan mode | Selected tile bound, capability present | Toggle, shows current state |
| Compact | Selected tile bound, capability present | Neutral |

Disabled keys stay visible with a tooltip stating *why* ("No pending request",
"Not supported in attached mode"). Capabilities marked `synthetic` by the
adapter render with a corner tick and the tooltip says what synthetic means.
A button that would do nothing silently is the one forbidden state.

## Dial

An arc with three click targets: minus, plus, and centre commit. The arc
sweep is a position indicator; dragging it works but is never required.

- Composer mode: minus and plus step across options (model, effort, permission
  mode), centre commits and advances.
- Pinned mode: the dial is bound to one option; default pin is the model.
- Current option name and value render inside the arc in full words, no
  abbreviation guessing.

## Stick

A four-way pad, rendered as a diamond of four 44 dip buttons: up (previous
tile), right (next tile), down (detail panel), left (previously selected
tile). Discrete clicks, no analog anything, no held repeat by default (hold
repeat is an opt-in).

## Talk and send

- Talk: click to start, click to stop, per the accessibility rules. While
  recording, a sea-green trace runs around the surface border, the device's
  travelling light translated to a bezel. Reduced motion: steady sea-green
  border. Hold-to-talk is opt-in.
- Send: adjacent to talk, enabled only when the selected session's adapter
  declares `send_prompt` and there is composed content to send. In attached
  mode this is normally disabled, and its tooltip explains why honestly.

## Layer strip

A thin row of dots or short labels, one per profile, single click to switch.
Layer 1 is Claude Code. The strip is the one place the 44 dip floor genuinely
binds; if labels do not fit, dots plus tooltip, plus the whole strip expandable
by one click.

## Detail panel

Expands from the surface (stick-down or click the selected tile's chevron).
Contents in order: session identity line (tile number, label, model, mode
badge, attached or hosted), the state in words, the current or pending item
(tool name and input for amber, last result line for green), Approve and Deny
when amber, context bar with percentage, cost figure, then Raise window,
Unbind, and per-session settings. The panel is also where every double-click
accelerator has its single-click equivalent (Raise window, most importantly).

Tool input display: monospace, wrapped, truncated past 12 lines with an expand
control. Truncation never disables the decision buttons.

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
