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
rows out of the list. [ADR-031](DECISIONS.md#adr-031), also the same day,
removed Move (the window is now repositioned by dragging only), made the
whole header its own drag region, shrank it to 52 px, relabelled and
restyled the grey toggle, and dropped the dashed outline unknown and
ended rows used to share. [ADR-033](DECISIONS.md#adr-033) (2026-09-14)
then replaced the grey toggle with a gear button that opens a settings
panel in place of the session list, described in its own section below.
[ADR-034](DECISIONS.md#adr-034), the next day, redrew the header, the
panel, and the session rows: icon buttons in place of text, toggle
switches beside their On/Off words, a combined Hooks and Repair row,
three titled sections grouping the panel, and rounded rows with a left
accent bar, plus a window-sizing fix so the open panel no longer shows
a scrollbar. Nothing it changed touches a frozen colour, a hit target,
or the row height floor; see that entry for what carries forward
unchanged.

Mockups in [`assets/`](../assets/), embedded in the README, predate both
and still draw the wider control set at the old sizes. They are drawings,
not screenshots, and where they disagree with this file, this file wins;
treat them as historical until they are redrawn.

## The surface

A frameless, always-on-top panel that never takes keyboard focus, laid out
as a single vertical list:

```
 ┌───────────────────────────────┐
 │  ◆1 ✕1 ◐1           ⚙    ✕   │  header, 52 px, 44 px+ targets
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
- Repositioned by dragging the window; anywhere on the header works,
  since the whole bar is the drag region. [ADR-031](DECISIONS.md#adr-031)
  removed the click-to-place alternative dragging used to have. This is
  an owner-approved exception to [ACCESSIBILITY.md](ACCESSIBILITY.md)'s
  no-required-drag rule, recorded there as an open gap, not as a
  compliant default.
- A saved position is checked against the monitors actually connected at
  startup, not trusted blindly, so a window last placed on a monitor that
  is no longer attached lands back inside the current work area instead of
  off every visible screen ([ADR-028](DECISIONS.md#adr-028)).
- Idle dim after 3 minutes without pointer interaction or state change,
  matching the device's lighting timeout. Any state change wakes it. Dim,
  not hide: a status board that hides is not a status board.
- Remembers position and scale.

## Header

Header height: 52 px, fixed ([ADR-031](DECISIONS.md#adr-031); 64 px
before it). The whole bar is the drag region, `data-tauri-drag-region`
on `#header` itself; there is no separate grip, and the count pills sit
on top of it with pointer events passed through, so a drag started on a
pill still drags the window. In order: a read-only summary of session
counts, the gear, and Quit, pinned to the header's right edge:

| Control | Width | Does |
| --- | --- | --- |
| Gear | 44 px | Opens or closes the settings panel in place of the session list, `aria-label="Settings"`, `aria-pressed` tracking whether it is open. Icon only: a cog closed, an arrow back to the session list open, on a raised, tinted background while open, so its pressed state is a shape and a background change, not only a colour ([ADR-034](DECISIONS.md#adr-034); ADR-033 first added the control and read its state as a text change instead). Always present, unlike the grey toggle it replaced, which used to hide itself when there was nothing to hide. |
| Quit | 44 px | Closes the daemon and the surface together. Icon only: an inline svg cross, `aria-label="Quit Deckhand"`, no visible text. |

The summary is a row of pills, one per state that currently has at least
one session in it, in a fixed order: waiting on you, error, thinking,
complete (idle, unknown, and ended are left to the rows). Each pill
pairs that state's glyph and colour with a count. It is read-only: it
reports, it does not select or filter, and it stays visible and
unchanged whether the session list or the settings panel is showing
underneath it.

## Settings panel

Opened and closed by the header's gear, in place of the session list
([ADR-033](DECISIONS.md#adr-033)). Not a second window: the same
window resizes to fit the panel's own row count through the daemon's
existing resize path, and closing restores the list's size. No control
here takes focus either; the window's no-activate behaviour is
unaffected by anything the panel does.

```
 ┌───────────────────────────────┐
 │  ◆1 ✕1 ◐1                ⬅  ✕ │  header, unchanged
 ├───────────────────────────────┤
 │ WINDOW                        │  section title
 │ Always on top          ⏻  On  │  panel row, 64 px, switch + word
 │ Start with Windows     ⏻  Off │
 │ Reset position              ↺ │  action row, two lines
 │ Moves the window back...      │
 │ LIST                          │  section title
 │ Hide unknown    ⏻  On, 3 hid. │
 │ CLAUDE CODE                   │  section title
 │ Hooks  [Installed]    Repair  │  status pill + button
 └───────────────────────────────┘
```

Five rows across three titled sections (Window, List, Claude Code),
each row still the same 64 px height as a session row, but a
different layout from a session row's: a text label, plus a switch, a
short description, or a status pill and a button, depending on the
row, never colour alone for any of them
([ADR-034](DECISIONS.md#adr-034); six flat rows before it, under
[ADR-033](DECISIONS.md#adr-033)). See
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#settings-panel) for what each
row does. The Hooks row is the one row with no click of its own; it
renders as a plain row rather than a button, the same way the header's
own summary does, so it never implies an action it does not have, but
it holds a real, separate Repair button rather than folding that
action into a second row the way ADR-033 first had it. The Repair
button is styled inactive rather than natively disabled when this
install has no checkout nearby to run the installer from: its result
text already reads "Installer not found," so a click while inactive is
an already-explained no-op, not a silent dead one
([ACCESSIBILITY.md](ACCESSIBILITY.md)).

Hide unknown is the header's former grey toggle, unchanged except for
where it lives: still the daemon's own `Registry.hide_unknown`, still
persisted, and a hidden session still reappears on its own, shifting
every target below it, the moment its state moves off `unknown`. Its
state text now names the hidden count directly, "On, 3 hidden" or
"Off," rather than folding it into a header button's own label. See
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#settings-panel) for the
trade-off that carries over unchanged from [ADR-030](DECISIONS.md#adr-030).

Move, the click-to-place alternative to dragging the window that used
to sit in the header, is removed ([ADR-031](DECISIONS.md#adr-031)); the
panel's own Reset position row (named "Reset window position" until
[ADR-034](DECISIONS.md#adr-034) shortened its on-screen label) moves
the window to a fixed default rather than restoring a click-based way
to place it anywhere; see [ACCESSIBILITY.md](ACCESSIBILITY.md) for the
open gap that leaves.

## Row anatomy

```
 ○  undertow
    IDLE
```

One row per session, two lines: a 30 px status glyph (colour plus shape) on
the left, the session name (17 px, bold) above the state word (uppercase),
filling the row's full width as a single click target. A Reveal miss note,
when present, sits on the row's second line, to the right of the state
word, one line, instead of overlapping it; the daemon's full sentence is
shortened to fit by `revealNote()` in `format.ts`
([ADR-031](DECISIONS.md#adr-031)).

- Row height: 64 px, fixed, above the
  [accessibility floor](ACCESSIBILITY.md#targets-and-sizing).
- Status is triple-coded on every row: colour, glyph, and the state word.
  See the state table in
  [ACCESSIBILITY.md](ACCESSIBILITY.md#status-without-colour).
- Every coloured state tints the row's background toward its colour and
  draws a 4 px accent bar down its left edge in the same colour
  ([ADR-034](DECISIONS.md#adr-034)): idle 5% (was 6%), thinking and
  complete 8% (was 14%), needs input and error 14% (was 22%), softer
  than before now that the accent bar carries part of the signal.
  Unknown and ended get no background tint, though the accent bar still
  reads their colour at full strength; a dashed outline used to mark
  them instead of either, removed by [ADR-031](DECISIONS.md#adr-031),
  and they are set apart from a live row by glyph shape and dimming.
  Selected: a 3 px inset outline in the text colour, on top of whatever
  tint the state already has.
- Unknown carries two words for the one state: "not heard yet" for a
  session bound by enumeration or restored from disk that no hook has
  spoken for yet this run, and "unknown" for one that spoke and then went
  quiet past `T_unknown`. Same colour, same glyph; only the word differs
  ([ADR-029](DECISIONS.md#adr-029)). Unknown rows
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
| Unknown | Grey, dimmed | Question | None |
| Ended | Off | Dash | None |

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
