# Control mapping: Codex Micro to Deckhand

Status: **accepted**. [ADR-028](DECISIONS.md#adr-028) narrowed the surface to
a session list on 2026-09-13 and removed the command keys, the stick, the
dial, talk, the detail panel, the bind picker, the layer strip, and the
corner badges that earlier versions of this file described.
[ADR-030](DECISIONS.md#adr-030), the same day, added a third header
control, Hide grey, back on top of that narrower surface.
[ADR-031](DECISIONS.md#adr-031), also 2026-09-13, then removed Move,
leaving the header with a grey toggle and Quit, and relabelled the
toggle. What follows describes the current design. The retabling and the
usage measurements that shaped the removed controls are preserved in the
ADRs that ADR-028 names as superseded, not restated here.

Deckhand is a software reimplementation of the [Codex Micro](https://learn.chatgpt.com/docs/features/codex-micro),
a limited-run macropad by Work Louder and OpenAI that acts as a command centre
for Codex chats. Deckhand keeps the device's core idea, a persistent status
board with one lamp per agent, and points it at Claude Code instead of the
ChatGPT desktop app.

This document is the source of truth for **what each control is**. It does not
say how a control is implemented; see [ARCHITECTURE.md](ARCHITECTURE.md) and
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) for that.

## Why clone a keyboard in software

The Codex Micro is a good design solving a problem it cannot fully solve. Its
value is not that it is a keyboard; it is that it is a **persistent, glanceable
status board with one lamp per agent**. Nothing about that requires a physical
object.

Putting it in software changes three things:

1. **It costs nothing and ships to everyone.** The hardware was a limited run.
2. **It is operable by pointer alone.** The device requires reaching, pressing,
   and holding twelve keys, a stick, and a dial. For the person writing this,
   that is the part that does not work. A pointer-driven surface is not a
   downgrade from the hardware, it is the only version that is usable at all.
3. **It can say more than one lit colour.** A physical LED is one bit of
   colour per key. A row can show a colour, a glyph, a session name, and the
   state in words, in the same line.

What is lost is real and should be stated plainly: no tactility, no muscle
memory, no operating it without looking, and it occupies screen space that
the hardware did not. See
[ACCESSIBILITY.md](ACCESSIBILITY.md#what-the-hardware-does-better).

## The mapping

### Agent keys to session rows

| Codex Micro | Deckhand |
| --- | --- |
| 6 frosted keys, each following one chat | One row per Claude Code session, in an ordered, unbounded list |
| Key LED shows chat status | Row colour, glyph, and a state word show session status |
| Press once: switch chat silently | Click once: select the session as Deckhand's target and raise its host window ([ADR-027](DECISIONS.md#adr-027)) |
| Press twice within 350 ms: switch and bring ChatGPT forward | The same single click. There is no double-click accelerator, since nothing is left for one to accelerate |
| Selected chat's key pulses with its status light | Selected row is marked; unselected rows are steady |
| Off means no assigned chat | No row at all: a session Deckhand has not yet seen is simply absent from the list |
| Six keys, a fixed count | Unbounded. A row is added the first time its session is seen and removed when the session ends or drops out of enumeration ([ADR-028](DECISIONS.md#adr-028)) |

Raising a window is the row's single click, not a double-press. The device's
350 ms double-press has no accelerator here because there is nothing left for
it to accelerate, which satisfies
[ACCESSIBILITY.md](ACCESSIBILITY.md#forbidden-interactions) by construction.
Deckhand's own window still never takes focus (ADR-025): the raise moves the
foreground to the session, never to the board.

The raise classifies the session's host before it decides how to look for
a window, because a pid means something different on each one
([ADR-032](DECISIONS.md#adr-032)). A plain console session's process owns
its window one to one, and Reveal finds it exactly by briefly attaching to
that console, no title guessing involved. A Windows Terminal session
shares one process, `WindowsTerminal.exe`, with every other window and tab
on the machine, so a pid only narrows "is this a Terminal window at all";
Reveal raises the single open Terminal window when there is exactly one,
and reports an honest miss, naming the ambiguity, when there is more than
one, rather than guess a tab. A VS Code session shares one main `Code.exe`
across every editor window the same way (three windows, one pid, observed
on 2.1.220), so Reveal instead reads the workspace folders VS Code's
Claude Code extension has open, matches the session's cwd against them,
and, on a match, runs VS Code's own CLI against that folder before raising
the window by title; a session in the browser, or one Reveal cannot place
on any host, is skipped with a logged reason rather than raising the wrong
thing. Across every host, two candidates tied for the best score is
treated the same as no match: Reveal never guesses between two equally
plausible windows. Deckhand's own window is excluded from the candidates a
match can land on ([ADR-028](DECISIONS.md#adr-028)). See
[DECISIONS.md](DECISIONS.md#adr-023) for the host axis this builds on and
[DECISIONS.md](DECISIONS.md#adr-032) for the full record, including why
tab-level targeting inside Windows Terminal or the session's own tab
inside a VS Code window is still out of reach from outside either editor.

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
| Off | No assigned chat | No row (session unseen or removed) |

Green specifically means **unread**. It clears when you select the row, which
is what makes the surface glanceable: anything not white is something you have
not dealt with.

Colour is never the only signal. Every state also has a distinct glyph and a
text label, because six-way colour coding fails for a large share of the people
this tool is for. See [UI_SPEC.md](UI_SPEC.md#state-rendering).

Amber can carry a `kind` (`permission` or `question`) at the protocol level;
no control on the current surface depends on it, so it does not change how a
row looks. See [ADR-028](DECISIONS.md#adr-028).

### Binding

Binding is automatic, not a picker. The first hook event or enumeration hit
for a session adds its row, at the end of the list.
[ADR-024](DECISIONS.md#adr-024) still holds: an enumeration alone binds and
names a row but leaves it `unknown` until an actual hook confirms a live
state. A row disappears when its session ends, or when it drops out of a
successful enumeration and has had no hook event for a while; a failed
enumeration removes nothing. Exact
timing lives in [ARCHITECTURE.md](ARCHITECTURE.md#observation-channels),
since it is a daemon behaviour, not a control. There is no bind picker and no
unbind action: nothing here is managed by hand.

### Header: the grey toggle and Quit

The header carries two controls: a grey toggle and Quit, pinned to the
header's right edge ([ADR-031](DECISIONS.md#adr-031); before it, three,
Hide grey, Move, and Quit, added by [ADR-030](DECISIONS.md#adr-030) on
top of [ADR-028](DECISIONS.md#adr-028)'s original two). The whole header
is now the drag region: there is no separate grip, and dragging from any
empty part of the bar, including the count pills, moves the window.

The grey toggle filters rows out of the list, not sessions out of the
daemon. A single click flips the setting. Off, the control reads "Hide
unknown." On, it shows pressed (its background and text colour change;
there is no inset outline) and reads "Show N unknown," where N is the
number of rows currently in the `unknown` state, "not heard yet" and past
`T_unknown` alike, so a hidden session is always counted and never simply
disappears ("Show unknown" when N is 0). If every bound session is
hidden, the list shows one placeholder row, "N unknown hidden," instead
of the normal list. The toggle disappears from the header entirely when
there is nothing unknown and hiding is already off, rather than sitting
there with no job; Quit's position at the right edge does not move when
it does. The header's own state-count summary is computed before this
filter and never changes when the toggle does.

The setting is the daemon's, not the surface's: it persists across
restarts, and a session that leaves the `unknown` state while hidden
reappears on its own, which moves every target below it without the
owner having clicked anything. Because `heard`
([ADR-029](DECISIONS.md#adr-029)) resets on every daemon restart, a
session that genuinely needed the owner before the restart also renders
`unknown` until a hook fires for it again, so hiding grey can hide a row
that needs a human; "Show N unknown" is the accepted mitigation for
that, not a fix for it. See [ADR-030](DECISIONS.md#adr-030) for the full
trade-off and the alternative, folding every grey row into one
expandable row, that was considered and not chosen.

Move, the click-to-place alternative to dragging the window that
[ADR-028](DECISIONS.md#adr-028) kept, is removed
([ADR-031](DECISIONS.md#adr-031)). The window is now repositioned by
dragging only; a saved position is still restored and clamped into the
current monitors' work area at startup, but there is no click-based way
to move it afterward. This is an owner-approved exception to
[ACCESSIBILITY.md](ACCESSIBILITY.md)'s no-required-drag rule, not a
reading that satisfies it; see ACCESSIBILITY.md for the open gap it
leaves. Quit closes the daemon and the surface together; it is icon-only
now, a cross glyph with `aria-label="Quit Deckhand"` and no visible
text.

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

### Removed from the surface

[ADR-028](DECISIONS.md#adr-028) removed the command keys (approve, deny,
answer, interrupt, continue, reveal), the stick, the dial, talk and send,
the detail panel that hosted several of them, the bind picker, the layer
strip, and the row's two corner badges. None of them had write authority.
Approve and deny are still Phase 2 work, but they are no longer planned to
land on this surface as designed here. Their per-control reasoning and the
usage measurements that shaped them live in the ADRs that ADR-028 names as
superseded (ADR-001's decision to keep the command keys, the stick, the
dial, and talk as controls; ADR-013's `kind` discriminator; ADR-019's tile
budget and corner badges) and are not restated here. A control returning to
the surface, approve and deny included, needs its own ADR, because ADR-028
is what removed the place it was going to go.

## Deliberate divergences

These are the places Deckhand knowingly does not copy the device. Each is a
decision with a reason, recorded here so it is not silently re-litigated.

| # | Divergence | Reason |
| --- | --- | --- |
| 1 | Single click selects and raises; the surface itself never takes focus | The device raises on a double press. Here the board exists to see every session and get to one, so one click does both, and the surface stays no-activate so the only focus change is to the session's window, never to Deckhand ([ADR-027](DECISIONS.md#adr-027)). |
| 2 | Status is never colour-only | Six-way colour coding is not readable for a large share of the target users. |
| 3 | Green clears on select, not on any interaction | Makes "not white" a reliable unread signal. |
| 4 | Binding is automatic and the list is unbounded, not a fixed six-slot picker | A session in another repo never showed up under the fixed six-slot design, because nothing rebound a slot on its own. An auto-binding, unbounded list fixes that ([ADR-028](DECISIONS.md#adr-028)). |

## Naming

The device calls them Agent Keys, Command Keys, the Dial, the Stick, the Mic
Key, and the Codex Key. Deckhand uses **rows** for the session list and
**the grey toggle** (labelled "Hide unknown" or "Show N unknown") and
**Quit** for the two header controls. Move had no device equivalent; it
existed only in Deckhand and was removed by
[ADR-031](DECISIONS.md#adr-031). None of the device's other names apply
to anything on the current surface.
