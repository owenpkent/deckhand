# UI specification

Status: **proposed**. This is the visual and interaction contract for the
surface. Behaviour semantics live in [CONTROL_MAPPING.md](CONTROL_MAPPING.md);
constraints on interaction live in [ACCESSIBILITY.md](ACCESSIBILITY.md) and
win any conflict with this file. [ADR-028](DECISIONS.md#adr-028) narrowed
the surface described here to a session list on 2026-09-13; the command
keys, dial, stick, talk, detail panel, bind picker, layer strip, and corner
badges an earlier version of this file specified are removed.

Mockups in [`assets/`](../assets/), embedded in the README, predate this
narrowing and still draw the wider control set. They are drawings, not
screenshots, and where they disagree with this file, this file wins; treat
them as historical until they are redrawn.

## The surface

A frameless, always-on-top panel that never takes keyboard focus, laid out
as a single vertical list:

```
 ┌───────────────────────────────┐
 │ Move            Quit          │  header, 44 px targets
 ├───────────────────────────────┤
 │ ○ undertow            idle    │  row, 48 px minimum
 │ ◐ contour          thinking   │
 │ ◆ deckhand   waiting on you   │
 │ ✓ meshview            done    │
 │ ? markcopy         unknown    │
 └───────────────────────────────┘
```

- About 360 logical pixels wide. Height follows the row count, each row at
  least 48 px, and the window is clamped into the monitor's work area so it
  can never render partly off-screen.
- Docks to any screen edge with a click-to-place move mode (no drag
  required; drag also works).
- A saved position is checked against the monitors actually connected at
  startup, not trusted blindly, so a window last placed on a monitor that
  is no longer attached lands back inside the current work area instead of
  off every visible screen ([ADR-028](DECISIONS.md#adr-028)).
- Idle dim after 3 minutes without pointer interaction or state change,
  matching the device's lighting timeout. Any state change wakes it. Dim,
  not hide: a status board that hides is not a status board.
- Remembers position and scale.

## Header

Exactly two controls, adjacent, each at least 44 px:

| Control | Does |
| --- | --- |
| Move | Enters click-to-place mode; click a destination to move the window there. No drag is required, though dragging the window also works. |
| Quit | Closes the daemon and the surface together. |

## Row anatomy

```
 ○ undertow                   idle
```

One row per session: a status glyph (colour plus shape), the session name,
and the state word, left to right, filling the row's full width as a
single click target.

- Row height: 48 px minimum, above the
  [accessibility floor](ACCESSIBILITY.md#targets-and-sizing).
- Status is triple-coded on every row: colour, glyph, and the state word.
  See the state table in
  [ACCESSIBILITY.md](ACCESSIBILITY.md#status-without-colour).
- Selected row: a visibly distinct outline or fill; pulse optional.
- Green (complete) clears to white when the row is *selected*, nothing
  else.
- There is no unbound row. A session Deckhand has not yet seen has no row
  at all; the list's length is the number of sessions currently bound. See
  [CONTROL_MAPPING.md](CONTROL_MAPPING.md#binding).

### The child ledger and COMPLETE

**COMPLETE is unreachable while the child ledger is non-empty.** Around 10.5%
of turns ended with children still running in the one corpus that has been
measured, 240 sessions on the author's machine, which is one user's habits and
not a general finding. On that machine a row that turns green on the parent's
stop is green while work continues, which falsifies the one promise the board
makes. A session whose parent turn has finished but whose ledger still has
entries stays in thinking, and goes green when the ledger empties. The count
of open entries is not currently shown anywhere in the UI: corner badges are
removed ([ADR-028](DECISIONS.md#adr-028)), and the gate itself is
authoritative in [ARCHITECTURE.md](ARCHITECTURE.md#the-child-ledger).

### State rendering

| State | Colour | Glyph | Motion (reduced-motion variant) |
| --- | --- | --- | --- |
| Idle | White | Open circle | None |
| Thinking | Blue | Arc spinner | Static arc glyph |
| Needs input | Amber | Hand | Slow breathe (static) |
| Complete | Green | Check | One pulse on entry (none) |
| Error | Red | Cross | None |
| Unknown | Grey, hatched | Question | None |
| Ended | Off | Dashed outline | None |

Glyphs are drawn, not emoji, so they render identically across platforms and
respect the theme.

The six colours and their meanings are frozen (ADR-008), so this table is not
where new states arrive. Amber can carry a `kind`, `permission` or `question`,
at the protocol level; it shares this row exactly, same colour, same glyph,
same label, and no control on the current surface reads it, so it never
changes how a row looks. See
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#status-colours).

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
several parallel agents that chirps on every completion trains you to mute
it.

## Empty and error states of the surface itself

- No daemon: the surface shows one full-width card saying the daemon is not
  reachable, with a start action. Never a row pretending to be a session.
- No sessions seen yet: the list is empty, with a one-line hint that
  Deckhand is watching for a session to start or to be enumerated. There is
  no picker to open, since binding is automatic.
- Adapter degraded (observing but not acting): a thin warning bar names the
  lost capability rather than letting the surface fail silently.
