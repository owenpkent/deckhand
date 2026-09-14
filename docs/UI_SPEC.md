# UI specification

Status: **proposed**. This is the visual and interaction contract for the
surface. Behaviour semantics live in [CONTROL_MAPPING.md](CONTROL_MAPPING.md);
constraints on interaction live in [ACCESSIBILITY.md](ACCESSIBILITY.md) and
win any conflict with this file. [ADR-028](DECISIONS.md#adr-028) narrowed
the surface described here to a session list on 2026-09-13; the command
keys, dial, stick, talk, detail panel, bind picker, layer strip, and corner
badges an earlier version of this file specified are removed.
[ADR-029](DECISIONS.md#adr-029), the same day, restyled what was left: 64
px two-line rows, header counts, a bundled typeface, and the other visual
changes this file now describes. [ADR-030](DECISIONS.md#adr-030), also the
same day, added a third header control, Hide grey, that filters `unknown`
rows out of the list.

Mockups in [`assets/`](../assets/), embedded in the README, predate both
and still draw the wider control set at the old sizes. They are drawings,
not screenshots, and where they disagree with this file, this file wins;
treat them as historical until they are redrawn.

## The surface

A frameless, always-on-top panel that never takes keyboard focus, laid out
as a single vertical list:

```
 ┌───────────────────────────────┐
 │ ⠿  ◆1 ✕1 ◐1   Hide Move Quit  │  header, 64 px, 44 px+ targets
 ├───────────────────────────────┤
 │ ○  undertow                   │  row, 64 px, two lines
 │    IDLE                       │
 │ ◐  contour                    │
 │    THINKING                   │
 │ ◆  deckhand                   │
 │    WAITING ON YOU             │
 │ ✓  meshview                   │
 │    COMPLETE                   │
 │ ?  markcopy                   │
 │    UNKNOWN                    │
 └───────────────────────────────┘
```

- About 360 logical pixels wide. Height follows the row count, each row a
  fixed 64 px, and the window is clamped into the monitor's work area so it
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

Header height: 64 px, fixed. In order: a drag grip (14 px), a read-only
summary of session counts, then three controls, each at least 44 px tall
([ADR-030](DECISIONS.md#adr-030); before it, two):

| Control | Width | Does |
| --- | --- | --- |
| Hide grey | 56 px | Toggles whether rows in the `unknown` state are shown. Reads "Hide" when off. When on, shows pressed (a 2 px inset outline) and reads "Show N," N being the count of currently hidden `unknown` rows. |
| Move | 48 px | Enters click-to-place mode; click a destination to move the window there. No drag is required, though dragging the window also works. |
| Quit | 48 px | Closes the daemon and the surface together. |

The summary is a row of pills, tightened to make room for the third
control, one per state that currently has at least one session in it, in
a fixed order: waiting on you, error, thinking, complete (idle, unknown,
and ended are left to the rows). Each pill pairs that state's glyph and
colour with a count. It is read-only: it reports, it does not select or
filter, and it is computed before the Hide grey filter, so it never
changes when that control is toggled.

When Hide grey is on and every bound session is hidden, the list shows
one placeholder row in place of the normal rows, "N grey hidden," styled
like the empty-list state rather than like a session row. A hidden row
reappears on its own, and every target below it shifts, the moment that
session's state moves off `unknown`; see
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#header-hide-grey-move-and-quit)
for the toggle's full behaviour and the trade-off it accepts.

## Row anatomy

```
 ○  undertow
    IDLE
```

One row per session, two lines: a 30 px status glyph (colour plus shape) on
the left, the session name (17 px, bold) above the state word (uppercase),
filling the row's full width as a single click target. A Reveal miss note,
when present, sits in its own column at the right of the row instead of
overlapping the state word, wrapping up to 3 lines before it truncates.

- Row height: 64 px, fixed, above the
  [accessibility floor](ACCESSIBILITY.md#targets-and-sizing).
- Status is triple-coded on every row: colour, glyph, and the state word.
  See the state table in
  [ACCESSIBILITY.md](ACCESSIBILITY.md#status-without-colour).
- Every coloured state tints the row's background toward its colour: idle
  6%, thinking and complete 14%, needs input and error 22%. Unknown and
  ended get a dashed outline instead of a tint. Selected: a 3 px inset
  outline in the text colour, on top of whatever tint or dashed outline
  the state already has.
- Unknown carries two words for the one state: "not heard yet" for a
  session bound by enumeration or restored from disk that no hook has
  spoken for yet this run, and "unknown" for one that spoke and then went
  quiet past `T_unknown`. Same colour, same glyph, same dashed outline;
  only the word differs ([ADR-029](DECISIONS.md#adr-029)). Unknown rows
  also dim, short of ended: the glyph and state word go to 75% grey toward
  the background, and the name drops from bold to regular weight at 72%
  text colour, since a not-heard-yet row may still be one really waiting
  on you.
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

## Typeface

Atkinson Hyperlegible Next, bundled (not a system font), drawn for
low-vision reading. The latin subset ships as a single variable-weight
woff2 at `app/ui/fonts/AtkinsonHyperlegibleNext-latin.woff2`, licensed
SIL OFL 1.1 (`app/ui/fonts/OFL.txt`). Anything outside that subset falls
back to Segoe UI. Previously the surface used the plain Segoe UI system
font throughout.

## Sound

Off by default. Optional short cues for amber and red only, distinct shapes,
each individually toggleable. Green and blue never make noise; a surface for
several parallel agents that chirps on every completion trains you to mute
it.

## Empty and error states of the surface itself

- No daemon: the surface shows one full-width card saying the daemon is not
  reachable, with a start action. Never a row pretending to be a session.
- No sessions seen yet: the list shows one row reading "Watching for
  sessions". There is no picker to open, since binding is automatic.
- Adapter degraded (observing but not acting): a thin warning bar names the
  lost capability rather than letting the surface fail silently.
