# Control mapping: Codex Micro to Deckhand

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
| Press twice within 350 ms: switch and bring ChatGPT forward | Double-click within 350 ms: select **and** raise that session's terminal window |
| Selected chat's key pulses with its status light | Selected tile pulses; unselected tiles are steady |
| Off means no assigned chat | Empty tile shows a bind affordance |

The 350 ms double-press window is taken from the device and kept deliberately.
It is configurable, because 350 ms is not reachable for every user; see
[ACCESSIBILITY.md](ACCESSIBILITY.md).

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

### Command keys

The device ships six customisable keys defaulting to fast mode, approve,
decline, continue, and send. Deckhand keeps six slots with Claude Code defaults:

| Slot | Default action | Notes |
| --- | --- | --- |
| 1 | Approve | Accept the pending permission request |
| 2 | Deny | Reject it |
| 3 | Continue | Send "continue" to a finished turn |
| 4 | Interrupt | Stop the current turn |
| 5 | Plan mode | Toggle plan mode for the selected session |
| 6 | Compact | Compact the selected session's context |

Approve and Deny are only enabled when the selected tile is amber. A button
that can approve a tool call is a security surface and is treated as one; see
[SECURITY_MODEL.md](SECURITY_MODEL.md).

### Stick

The device's analog stick moves freely and maps four directions. Deckhand uses
a four-way pad because a pointer cannot hold an analog deflection comfortably.

| Direction | Device | Deckhand |
| --- | --- | --- |
| Up | Plan mode | Previous tile |
| Right | Forward history | Next tile |
| Down | Sidebar toggle | Expand or collapse the detail panel |
| Left | Back history | Return to the previously selected tile |

Plan mode moves to a command key because Deckhand has a spare slot and tile
navigation is the more frequent need on a surface with six sessions.

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
| 4 | Plan mode moves from stick to command key | Freed the stick for tile navigation, which is more frequent with six sessions. |
| 5 | Status is never colour-only | Six-way colour coding is not readable for a large share of the target users. |
| 6 | Green clears on select, not on any interaction | Makes "not white" a reliable unread signal. |
| 7 | No speech recognition in this repo | MacroVox already does it. |

## Naming

The device calls them Agent Keys, Command Keys, the Dial, the Stick, the Mic
Key, and the Codex Key. Deckhand uses **tiles**, **command keys**, **dial**,
**stick**, **talk**, and **send**. "Key" is kept where the thing is a button
you press and dropped where it is not, since none of these are keys on a
keyboard any more.
