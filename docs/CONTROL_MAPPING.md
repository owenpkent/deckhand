# Control mapping: Codex Micro to Deckhand

Status: **accepted**. The command keys, the stick, and Reveal were retabled on
2026-07-30 against measured Claude Code usage. The measurement column in each
table is the evidence, and it is one user's corpus on one machine, not a
general finding.

Deckhand is a software reimplementation of the [Codex Micro](https://learn.chatgpt.com/docs/features/codex-micro),
a limited-run macropad by Work Louder and OpenAI that acts as a command centre
for Codex chats. Deckhand keeps the device's interaction model and points it at
Claude Code instead of the ChatGPT desktop app.

This document is the source of truth for **what each control is**. It does not
say how a control is implemented; see [ARCHITECTURE.md](ARCHITECTURE.md) and
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) for that.

## Why clone a keyboard in software

The Codex Micro is a good design solving a problem it cannot fully solve. Its
value is not that it is a keyboard; it is that it is a **persistent, glanceable
status board with one lamp per agent**, plus a small set of always-available
actions. Nothing about that requires a physical object.

Putting it in software changes three things:

1. **It costs nothing and ships to everyone.** The hardware was a limited run.
2. **It is operable by pointer alone.** The device requires reaching, pressing,
   and holding twelve keys, a stick, and a dial. For the person writing this,
   that is the part that does not work. A pointer-driven surface is not a
   downgrade from the hardware, it is the only version that is usable at all.
3. **It can show more than six colours.** A physical LED is one bit of colour
   per key. A tile can show status, elapsed time, the current tool, the repo
   name, and a diff count in the same footprint.

What is lost is real and should be stated plainly: no tactility, no muscle
memory, no operating it without looking, and it occupies screen space that the
hardware did not. See [ACCESSIBILITY.md](ACCESSIBILITY.md#what-the-hardware-does-better).

## The mapping

### Agent keys to agent tiles

| Codex Micro | Deckhand |
| --- | --- |
| 6 frosted keys, each following one chat | 6 tiles, each bound to one Claude Code session |
| Key LED shows chat status | Tile background and status ring show session status |
| Press once: switch chat silently | Click once: select the session as Deckhand's target, do not touch window focus |
| Press twice within 350 ms: switch and bring ChatGPT forward | Click the tile's Reveal target: raise that session's host window. Double-click does the same as an off-by-default accelerator |
| Selected chat's key pulses with its status light | Selected tile pulses; unselected tiles are steady |
| Off means no assigned chat | Empty tile shows a bind affordance |

Raising a window is a single click on a target, not a double-press. The
device's 350 ms double-press survives as an accelerator that is off by default
and has an adjustable window, which is what
[ACCESSIBILITY.md](ACCESSIBILITY.md#forbidden-interactions) already requires of
every double-click here. Nothing is reachable only by double-clicking.

Reveal matches a session to a host by pid where one is available, since
`claude agents --json` reports a `pid` per live session (observed on Claude
Code 2.1.220). Window-title matching is the fallback and is a heuristic.
Sessions also run in editors and in the browser, and Deckhand can only raise a
local window, so Reveal is disabled with a stated reason whenever the host is
not locatable rather than raising the wrong thing.

### Status colours

Taken from the device unchanged, so anyone who has used a Codex Micro already
knows how to read a Deckhand surface.

| Colour | Device meaning | Deckhand meaning |
| --- | --- | --- |
| White | Idle | Session is alive and waiting for you |
| Blue | Thinking | Claude is working: generating or running a tool |
| Green | Complete | Turn finished, you have not looked yet |
| Amber | Requires input | Blocked on a permission decision or a question |
| Red | Error | The turn failed, or the session crashed |
| Off | No assigned chat | Tile is unbound |

Green specifically means **unread**. It clears when you select the tile, which
is what makes the surface glanceable: anything not white is something you have
not dealt with.

Colour is never the only signal. Every state also has a distinct glyph and a
text label, because six-way colour coding fails for a large share of the people
this tool is for. See [UI_SPEC.md](UI_SPEC.md#state-rendering).

### Amber carries a kind

Amber stays one colour, one glyph, one label. ADR-008 is untouched: no new
state, no new colour, nothing repurposed. What is new is a discriminator on the
update. `SessionUpdate.detail` carries `kind: "permission" | "question"`. A
permission is a tool call waiting on a decision. A question is a
multiple-choice prompt: 322 `AskUserQuestion` calls across 155 of 240 local
sessions, against 10 to 27 user denials in the same corpus. Most amber is not
a permission at all.

The kind decides which controls light, not what the tile looks like:

- Approve and Deny are enabled only when `kind` is `permission`. Lighting
  Approve on a question would be a button that cannot do what its label says,
  which is the one state [UI_SPEC.md](UI_SPEC.md#command-keys) forbids.
- Answer is enabled only when `kind` is `question`. Each option gets its own
  target showing the full option label, never a bare letter or index, at the
  44 px floor and with the half-target dead gap between adjacent targets from
  [ACCESSIBILITY.md](ACCESSIBILITY.md#targets-and-sizing). See
  [UI_SPEC.md](UI_SPEC.md#answer-targets).

The capability behind Answer is `answer_question`, and it is **optional and
unproven**: no answer channel has been observed on a live install, so the
control ships disabled and says so until one is. MCP elicitation is a separate
prompt shape, MCP-only, and it is not a candidate: it gets no kind, no glyph,
and no control.

### Command keys

The device ships six customisable keys defaulting to fast mode, approve,
decline, continue, and send. Deckhand keeps six slots. The Claude Code defaults
are ranked by how often a human actually needs the action, measured across 240
local interactive sessions on 2026-07-30. One machine, one user's habits: the
numbers make the ranking auditable rather than asserted, and they are not a
general finding.

| Slot | Default action | Measured over 240 sessions | Enabled when |
| --- | --- | --- | --- |
| 1 | Approve | Not counted: the corpus records denials, not approvals. Read slot 2 as the floor for both, since the classifier answered the rest | Selected tile amber and `kind` is `permission` |
| 2 | Deny | 10 to 27 user denials | Selected tile amber and `kind` is `permission` |
| 3 | Answer | 322 `AskUserQuestion` calls in 155 of 240 sessions, 2 to 4 options each | Selected tile amber and `kind` is `question` |
| 4 | Interrupt | 55 to 62 interrupts | Selected session thinking |
| 5 | Continue | 82 bare continuation prompts, 3.0% of typed input | Selected tile complete **and** a send channel exists |
| 6 | Reveal | Not counted; needed once per "where is that session" | Selected tile bound to a locatable host |

Plan mode and Compact held two of these slots and are demoted to the detail
panel. Plan mode produced two `ExitPlanMode` events in 240 sessions, and both
plan-mode hooks are model-invocable, so it was never Deckhand's latch to hold.
Manual compaction produced 5 to 10 markers, and auto-compaction does the rest.
Neither earns a permanent slot at roughly one use per 500 sessions. There is no
overflow shelf and no second page: the detail panel already exists and is one
click away.

Continue is the honest problem in this table. Sending "continue" needs a write
channel into a running session, and Deckhand does not have one in attached
mode: no observed interface delivers a prompt into an idle session, which is
exactly when a person wants to send one. Continue therefore ships **disabled
with a stated reason**, or as `synthetic` if the user opts in to a typing shim.
It is not specified as a button that works, because it does not.

Approve and Deny are a security surface and are treated as one; see
[SECURITY_MODEL.md](SECURITY_MODEL.md). Permission mode is part of what enables
them. Deckhand's `PreToolUse` gate runs first in every mode, and a hook `allow`
still runs in `dontAsk`, so the gate is not dead in any mode. What differs is
where a Deckhand `ask` lands: on a human in `manual`, on the
classifier in `auto`, on a denial in `dontAsk`. Where
the mode means no human decision is coming, Approve and Deny are disabled and
the mode is named as the cause. The tile carries the mode as a text badge,
never as a colour; see [UI_SPEC.md](UI_SPEC.md#tile-budget).

Every disabled key states why. Clicking one is never a no-op: it reveals the
reason in the detail panel, naming the cause (no pending request, wrong kind,
permission mode, missing capability, no send channel), and expands the panel if
it is collapsed. No reason lives in a tooltip.

### Stick

The device's analog stick moves freely and maps four directions. Deckhand uses
a four-way pad because a pointer cannot hold an analog deflection comfortably.

| Direction | Device | Deckhand |
| --- | --- | --- |
| Up | Plan mode | Scroll the detail panel up, over the pending tool input |
| Right | Forward history | Expand or collapse the detail panel |
| Down | Sidebar toggle | Scroll the detail panel down |
| Left | Back history | Return to the previously selected tile |

Tile stepping is gone. It was justified here as the more frequent need on a
surface with six sessions, and the measurement says otherwise: one interactive
session is live 47.6% of the time, two or fewer 76.4% of the time, and six or
more only 0.5% of the time. Two of four stick targets were stepping a list that
usually has one item, to reach a 96 dip tile that was already one click away.

What had no cheap pointer path was reading a long tool input before deciding on
it, which is the interaction this surface exists for. Up and down now scroll
that text in the detail panel, so a decision on a forty-line command costs
clicks on a 44 px target instead of a wheel or a drag. That leaves four
distinct jobs on four directions, none of them a duplicate of a click on a
tile.

### Dial

On the device the dial scrolls composer options with Reasoning selected by
default, and presses to select. Deckhand keeps both behaviours and both modes:

- **Composer mode**: step through the adjustable options for the selected
  session, press to commit.
- **Pinned mode**: the dial is pinned to one option, mirroring the device's
  "Reasoning only" setting. Default pin is the model.

A pointer cannot turn a dial, so the dial renders as an arc with a minus and a
plus target flanking a centre commit target. The arc is decorative; the targets
do the work. Dragging around the arc is supported but never required.

The default pin stays the model, because that is the option the corpus actually
touches: 28 uses of `/model` against 5 of `/effort` across the same 240
sessions.

In attached mode the dial is a **readout, not a control**. No observed local
interface sets the model, the effort level, or the permission mode of a running
session, so the dial reports what the session says it is using and its commit
target is disabled with that reason. Hosted mode is where it gains a write
channel; see [ARCHITECTURE.md](ARCHITECTURE.md#hosted-mode).

Two guardrails on the steppers, whatever the dial is pinned to. They never
reach `dontAsk` or `bypassPermissions`: those values are outside the stepper's
range, and getting to either is a deliberate action elsewhere, never one click
next to a plus and a minus. And thinking budget and context budget are
readouts, never dial values.

### Talk

The device's mic key is push-to-talk on the computer's microphone, with a
double-press within 350 ms for hands-free, and a sea-green light travelling
around the keyboard while recording.

Deckhand keeps push-to-talk, hands-free toggle, and the travelling recording
indicator. It does **not** implement speech recognition: that already exists in
MacroVox, the sibling tool in this ecosystem, and duplicating it would be a
second thing to maintain and a second place for audio to leak. Talk delegates.
See [ROADMAP.md](../ROADMAP.md) for the phase this lands in.

Push-to-talk on a pointer means holding a mouse button, which is exactly the
kind of sustained input this project exists to avoid. Talk therefore defaults
to **click to start, click to stop**, with hold-to-talk available for those who
prefer it. This inverts the device's default on purpose.

### Send

The device's Codex key sends the composed message. Deckhand's Send does the
same for the selected session.

In attached mode Send is normally disabled and the adapter declares
`send_prompt` as `false` there: no observed interface delivers a prompt into an
idle Claude Code session. Clicking the disabled key states that reason. See
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

### Touch control, layers, and channels

The device's bottom-left touch sensor cycles layers, holds three seconds for
Bluetooth pairing, and exposes three BLE channels. Work Louder's Input app adds
up to six layers with app-detection.

Deckhand has no radio and needs no pairing, so this collapses to a **layer
strip**: a row of small targets that switch the whole surface between profiles.
Layer 1 is Claude Code. Other layers are user-defined and can target any
adapter. App-linked auto-switching is a later phase.

### What has no software equivalent

| Device feature | Disposition |
| --- | --- |
| Bluetooth pairing, 3 channels | Dropped. No radio. |
| USB-C wired mode | Dropped. |
| Battery reporting | Dropped. |
| Rear power button, sleep | Dropped. Window visibility replaces it. |
| Underglow, lighting timeout (default 3 min) | Kept as an idle-dim behaviour on the surface. |
| macOS Input Monitoring permission | Not required to read status. Required only for global hotkeys, which are optional. |
| Soft reset via PCB screws | Replaced by a settings reset. |
| Karabiner and Logitech Options conflicts | Not applicable. |

## Deliberate divergences

These are the places Deckhand knowingly does not copy the device. Each is a
decision with a reason, recorded here so it is not silently re-litigated.

| # | Divergence | Reason |
| --- | --- | --- |
| 1 | Single click never changes window focus | On a shared screen the surface must not steal focus from the terminal it is driving. The device is a separate object and does not have this problem. |
| 2 | Talk defaults to click-to-toggle, not hold | Sustained holds are the specific thing this project exists to remove. |
| 3 | Stick is four-way discrete, not analog | A pointer cannot comfortably hold a deflection. |
| 4 | Plan mode leaves the stick, and the stick does not navigate tiles | Plan mode fired twice in 240 sessions and both plan hooks are model-invocable, so it earns neither a stick direction nor a permanent key; it lives in the detail panel. Tile stepping went with it: six or more concurrent sessions happens 0.5% of the time, so stepping reached a tile that was already one click away. The stick scrolls the pending tool input instead. |
| 5 | Status is never colour-only | Six-way colour coding is not readable for a large share of the target users. |
| 6 | Green clears on select, not on any interaction | Makes "not white" a reliable unread signal. |
| 7 | No speech recognition in this repo | MacroVox already does it. |
| 8 | Command keys are retabled around answering, not the device's mode toggles | The device defaults to fast mode, approve, decline, continue, and send. Measured usage puts questions at 322 against plan mode at 2, so Answer and Reveal take the slots plan mode and compact held. |
| 9 | Raise becomes Reveal, on a single click | The device raises on a double press. Here a double-click is an accelerator and never the only route, so Reveal is a single-click target on the tile and in the detail panel. |

## Naming

The device calls them Agent Keys, Command Keys, the Dial, the Stick, the Mic
Key, and the Codex Key. Deckhand uses **tiles**, **command keys**, **dial**,
**stick**, **talk**, and **send**. "Key" is kept where the thing is a button
you press and dropped where it is not, since none of these are keys on a
keyboard any more.
