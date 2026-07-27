# Accessibility

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
- **One click per decision.** Select tile, approve, deny, continue: each is a
  single press on a static target.
- **Short travel.** Controls that act on the selected tile sit adjacent to the
  tile strip, not across the window. The surface docks to a screen edge so it
  lives near wherever the pointer already works.
- **No interaction taxes.** No hover-to-reveal with a timeout, no drag-only
  controls, no scroll-to-reach-the-button, no confirmation dialogs that appear
  at a different screen position than the action that raised them.

## Forbidden interactions

These may not ship as the only way to do anything:

| Forbidden as sole path | Because | Provided alternative |
| --- | --- | --- |
| Press-and-hold | Sustained force is the exact cost being avoided | Click-to-toggle (talk defaults to this) |
| Drag | Sustained force plus precision | Dial has click targets; windows move via a move mode, click destination |
| Double-click | Timing windows exclude dwell clickers | Every double-click action also exists as a single-click affordance (select tile, then a Raise button in the detail panel) |
| Hover-only reveals | Dwell users cannot hover without clicking | Everything visible is clickable; nothing important hides behind hover |
| Keyboard input | The whole premise | Text entry delegates to the system keyboard of choice, for example alpha-osk; naming things is optional everywhere |
| Chorded or simultaneous inputs | One pointer, one action | Never used |

The 350 ms double-click window inherited from the Codex Micro survives as an
*optional accelerator* with an adjustable window (up to 2000 ms) and an off
switch, because for a mouse user it is genuinely faster, and for everyone else
it must not be load-bearing.

## Targets and sizing

- **Minimum hit target: 44 by 44 device-independent pixels** for anything
  interactive, measured at 100% surface scale. WCAG 2.2 asks 24 at AA and 44
  at AAA; Deckhand takes the AAA number as its floor and treats it as a build
  constant, not a guideline (the PR template asks about it by name).
- Default tiles are much larger than the floor. The floor exists for the layer
  strip and dial steppers, the smallest things on the surface.
- **Surface scale from 100% to 300%**, everything scaling together. At 300% on
  a 1080p screen, four tiles and the command keys must still fit; if a layout
  cannot survive that, the layout is wrong.
- Adjacent destructive and constructive controls (Approve next to Deny) get
  a mandatory gap of at least half a target width, so a tremor miss lands on
  dead space, not the opposite decision.

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
| Unknown | Grey | Question, hatched fill | Unknown |
| Ended or unbound | None | Dashed outline | Empty |

Plus: a high-contrast theme, glyph-only mode for monochrome displays, reduced
motion mode (spinners become static badges; the talk sweep becomes a steady
border), and adjustable pulse behaviour, since the selected tile's pulsing is
information for some and noise for others.

## What the hardware does better

Honesty section, referenced from [CONTROL_MAPPING.md](CONTROL_MAPPING.md). The
Codex Micro beats a software surface at: tactile confirmation, operation
without looking, muscle memory across days, zero screen footprint, and working
while your pointer is busy elsewhere. Deckhand accepts all five losses because
the hardware's own cost, requiring functioning hands, is the one this project
cannot pay. Anyone who can use the device happily should; the two are not in
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
[SECURITY_MODEL.md](SECURITY_MODEL.md)) is an informed default, and
feedback from disabled users outranks every one of them. The
[accessibility feedback template](https://github.com/owenpkent/deckhand/issues/new?template=accessibility_feedback.yml)
never requires disclosing a diagnosis: describe what is hard, not why.
