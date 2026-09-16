# Accessibility

Status: **accepted**. Unlike the design documents, this one is not waiting on
evidence. The constraints below are inputs to the design, not proposals about
it, and they win every conflict with the other specs.

This is not the compliance appendix. It is the requirements document the rest
of the project answers to. Deckhand exists because its author, a wheelchair
user with muscular dystrophy, can move a pointer far more cheaply than he can
press keys, and every design rule below follows from taking that seriously.

## The one rule

**Every feature must be fully operable with a pointer alone.** Single clicks
on stationary targets, nothing else required. Keyboard shortcuts and voice are
welcome as conveniences; the moment one becomes the only way to do something,
that is a release-blocking bug, not a polish item.

"Pointer" is deliberately broad: mouse, trackball, head pointer, eye tracker
with dwell clicking, joystick-driven cursor. The cheapest common denominator
across all of those is the **single click on a target that is not moving**, so
that is the primitive everything must reduce to.

## The economics

The point of a status board is saving motion. The baseline it competes with,
checking six terminal windows by alt-tab or taskbar, costs window switches,
visual reorientation, and often scrolling, dozens of times an hour. For someone
with limited strength, that cost decides how many parallel sessions are usable
at all.

So the surface is judged on motion arithmetic:

- **Glance beats click.** Any state that matters must be readable with zero
  interaction. That is why unread is a colour and not a badge you hover for.
- **One click per decision.** Selecting a session is a single press on a
  static target today; approve, deny, and answering a question will be too
  whenever they return to the surface (they are Phase 2 or later, and
  currently off it, see [ADR-028](DECISIONS.md#adr-028)).
- **Short travel.** The session list sits together as one control and stays
  where it was last placed, near wherever the pointer already works
  (placing it is currently drag-only, see the exception below).
- **No interaction taxes.** No hover-to-reveal with a timeout, no drag-only
  controls, no scroll-to-reach-the-button, no confirmation dialogs that appear
  at a different screen position than the action that raised them.
- **Reading is motion too.** Whenever a future decision requires reading a
  tool call's input, that reading has to be reachable without a wheel, a
  drag, or a keyboard, the same rule that shaped the stick before ADR-028
  removed it along with the panel it scrolled. The requirement carries
  forward even though the controls that were going to satisfy it do not
  currently exist: reading must never be *required* before a decision is
  usable, only offered as a click.
- **Never manufacture a click.** A feature that creates more presses than it
  removes is a regression, however cheap each press is. This is why the
  permission gate ships scoped to shell execution and file deletion rather than
  matching every tool call: a gate on everything would turn a quiet session
  into one amber per tool call, and Deckhand would be the source of the cost it
  exists to remove.

The largest single saving on offer is not approval, it is **answering**. In the
one corpus that has been measured, 240 sessions on the author's machine, a
multiple-choice question from the agent was roughly fifteen times more common
than a tool denial, and answering one costs a window switch, a read, and a
keypress today. Collapsing that into one click on a labelled target is the
strongest accessibility argument in the whole design. One machine's habits are
not a general finding and the claim is scoped to that, but on this machine it
is not close. The current surface does not implement an answer control
([ADR-028](DECISIONS.md#adr-028)); this argument is preserved as the case for
one returning, not as a description of what ships today.

## Forbidden interactions

These may not ship as the only way to do anything:

| Forbidden as sole path | Because | Provided alternative |
| --- | --- | --- |
| Press-and-hold | Sustained force is the exact cost being avoided | Click-to-toggle, used wherever a sustained action would otherwise be required |
| Drag | Sustained force plus precision | Repositioning the window is a named exception (see below); every other drag on the surface remains optional |
| Double-click | Timing windows exclude dwell clickers | Nothing is double-clicked: a row click selects and raises in one click, and every other action is a single click on its own target |
| Hover-only reveals | Dwell users cannot hover without clicking, and the surface never takes focus, so there is no keyboard route to a tooltip either | Everything visible is clickable, and anything a control needs to explain itself is shown in the open, never gated behind a hover |
| Keyboard input | The whole premise | Text entry delegates to the system keyboard of choice, for example alpha-osk; naming things is optional everywhere |
| Chorded or simultaneous inputs | One pointer, one action | Never used |

**Drag is no longer optional for one action.**
[ADR-031](DECISIONS.md#adr-031) removed Move, the click-to-place
command that had been the required alternative to dragging the
window, at the owner's explicit request; the owner is also the
mouse-only user this rule exists to protect, and accepted the trade-off
knowingly. Repositioning the window now has no click-based route: a
saved position is still restored and clamped into the currently
connected monitors' work area at startup ([ADR-028](DECISIONS.md#adr-028)),
so the window never starts off-screen, but nothing short of a drag
can move it once it is placed. This is a real, open accessibility
gap, not a compliant reading of the rule above, and it stands until a
click-based reposition control returns, which would need its own ADR.

The 350 ms double-click inherited from the hardware macropad has no accelerator
here: since [ADR-027](DECISIONS.md#adr-027) the row's single click already
raises, so there is nothing for a double-click to be faster at. If one ever
returns for some other action it must be optional, adjustable (up to 2000 ms),
switchable off, and never load-bearing.

Two consequences of the hover row are load-bearing enough to state outside the
table.

**A tooltip is not a slow reveal here, it is no reveal at all.** The surface
never takes keyboard focus (see [UI_SPEC.md](UI_SPEC.md)), so a tooltip has
neither a hover route for a dwell or eye-tracker user nor a focus route for
anyone else. Any text a control needs in order to be understood has to be
shown directly, as label text on the control itself, never behind a hover.

**Clicking a disabled control is never a no-op.** It reveals why the control is
disabled: nothing pending, the wrong kind of amber, a permission mode in which
the decision would not have reached you, or a channel Deckhand does not have.
A control that neither acts nor explains teaches its user to distrust their own
click, and a doubted click gets repeated, which costs more than the action ever
would have. No control on the current surface is a native, natively-disabled
element; the settings panel's Repair button
([ADR-033](DECISIONS.md#adr-033), moved into the combined Hooks row's
own button by [ADR-034](DECISIONS.md#adr-034)) is the first control
this rule actually binds, and it is met by never disabling the button
at all: when there is no installer to run, it is styled inactive but
stays clickable, and its reason, "Installer not found," is already its
row's permanent, always-visible secondary text rather than something a
click would have to reveal. The rule binds whatever control is added
next.

## Targets and sizing

- **Minimum hit target: 44 by 44 device-independent pixels** for anything
  interactive, measured at 100% surface scale. WCAG 2.2 asks 24 at AA and 44
  at AAA; Deckhand takes the AAA number as its floor and treats it as a build
  constant, not a guideline (the PR template asks about it by name).
- Session rows are larger than the floor: 64 px, fixed, per
  [UI_SPEC.md](UI_SPEC.md#row-anatomy). The floor binds hardest on the
  header, now 52 px total ([ADR-031](DECISIONS.md#adr-031)): Quit and
  the gear are both 44 by 44 px icon buttons
  ([ADR-034](DECISIONS.md#adr-034); the gear was a variable-width text
  button before it), exactly the floor on every side. The settings
  panel's own rows
  ([ADR-033](DECISIONS.md#adr-033)) reuse the 64 px row height rather
  than sitting at the floor.
- **Surface scale from 100% to 300%**, everything scaling together. At 300%
  on a 1080p screen, the header and at least a few rows must still render
  legibly; if a layout cannot survive that, the layout is wrong.
- **Standing rule for any future control:** adjacent destructive and
  constructive controls get a mandatory gap of at least half a target
  width, so a tremor miss lands on dead space, not the opposite decision.
  Approve and Deny are the anticipated case (Phase 2) and are not
  currently on the surface to apply it to.
- **Standing rule for any future question control:** a session's
  multiple-choice options are targets, not a legend. Every option would get
  its own hit target carrying the full option label, never a bare letter or
  an index the user has to map back to something else, at the 44 px floor
  with the same half-target dead gap. No answer control is currently on the
  surface ([ADR-028](DECISIONS.md#adr-028)).

## Status without colour

Six states distinguished only by hue would fail a large fraction of users, so
colour is the fastest channel, never the only one:

| State | Colour | Glyph | Label |
| --- | --- | --- | --- |
| Idle | White | Open circle | Idle |
| Thinking | Blue | Arc spinner | Working |
| Needs input | Amber | Hand | Waiting on you |
| Complete | Green | Check | Done, unread |
| Error | Red | Cross | Problem |
| Unknown | Grey | Question | Not heard yet, or Unknown |
| Ended or unbound | None | Dash | Empty |

Unknown carries two labels for the one state: "Not heard yet" for a session
bound by enumeration or restored from disk that no hook has spoken for yet
this run, and "Unknown" for one that had spoken and then gone quiet past
`T_unknown`. Colour and glyph are identical between the two; only the word
differs, so the distinction still reaches glyph-only and colour-blind modes
without adding a channel ([ADR-029](DECISIONS.md#adr-029)). Unknown and
ended rows no longer carry a shared dashed outline either; that treatment
was removed by [ADR-031](DECISIONS.md#adr-031), which leaves the two
states to read apart from a live row by glyph shape and dimming alone.
Unknown rows are also dimmer than the rest of the list, though deliberately
less dim than an ended row: glyph and state word go to 75% grey toward the
background, and the name drops from bold to regular weight at 72% text
colour.

Amber can carry a kind, a permission request or a question, at the protocol
level. Should a future control ever key off it, that distinction would have
to reach the label, not only the enabled buttons: someone reading the board
in glyph-only mode would still need to know what the next click does. Today
no control on the surface reads kind, so it does not change the row. It is
the same colour, the same glyph, and the same state either way, so
[ADR-008](DECISIONS.md#adr-008) is untouched.

Plus: a high-contrast theme, glyph-only mode for monochrome displays, reduced
motion mode (spinners become static glyphs), and adjustable pulse behaviour,
since the selected row's pulsing is information for some and noise for
others.

## What the hardware does better

Honesty section, referenced from [CONTROL_MAPPING.md](CONTROL_MAPPING.md). The
device beats a software surface at: tactile confirmation, operation without
looking, muscle memory across days, zero screen footprint, and working while
your pointer is busy elsewhere. Deckhand accepts all five losses because the
hardware's own cost, requiring functioning hands, is the one this project cannot
pay. Anyone who can use the device happily should; the two are not in
competition.

## Screen readers and switch access

Stated plainly rather than promised vaguely:

- The surface will carry a correct accessibility tree (roles, names, states)
  because Tauri's webview makes that achievable and there is no excuse not to.
  But Deckhand is a visual glance-board first; a screen-reader-first
  equivalent would be a different, also worthwhile design (status changes as
  announcements rather than colours). The tree is a floor, not the product.
- Switch access and scanning are not implemented. The intended route is the
  sibling project Nimbus (virtual joystick) driving the pointer, plus an
  eventual scanning layer listed in [IDEAS.md](../IDEAS.md). Until built, the
  claim is only "planned".

## Feedback outranks estimates

Every number above (44 px, 350 ms, 300%, plus the 500 ms rule in
[SECURITY_MODEL.md](SECURITY_MODEL.md) and the 900 s `T_unknown` deadline in
[ARCHITECTURE.md](ARCHITECTURE.md#liveness-by-open-operation)) is an informed
default, and feedback from disabled users outranks every one of them. The
[accessibility feedback template](https://github.com/owenpkent/deckhand/issues/new?template=accessibility_feedback.yml)
never requires disclosing a diagnosis: describe what is hard, not why.
