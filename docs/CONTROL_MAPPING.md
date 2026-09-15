# Control mapping: Codex Micro to Deckhand

Status: **accepted**. [ADR-028](DECISIONS.md#adr-028) narrowed the surface to
a session list on 2026-09-13 and removed the command keys, the stick, the
dial, talk, the detail panel, the bind picker, the layer strip, and the
corner badges that earlier versions of this file described.
[ADR-030](DECISIONS.md#adr-030), the same day, added a third header
control, Hide grey, back on top of that narrower surface.
[ADR-031](DECISIONS.md#adr-031), also 2026-09-13, then removed Move,
leaving the header with a grey toggle and Quit, and relabelled the
toggle. [ADR-033](DECISIONS.md#adr-033) (2026-09-14) then added a
fourth header control, a gear, that opens a settings panel in place of
the session list, holding always on top, start with Windows, reset
position, and a hooks status with a Repair action; the grey toggle
stayed a header control throughout. [ADR-034](DECISIONS.md#adr-034),
the next day, restyled the grey toggle as a switch and shortened its
wording to "Hide grey" / "N hidden". What follows describes the current
design. The retabling and the usage measurements that shaped the
removed controls are preserved in the ADRs that ADR-028 names as
superseded, not restated here.

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
state. A row's name is the scan's own `name` when the scan has provided one,
and the directory name otherwise; a directory-derived name stays open to a
later scan name, while a real name, once seen, is never replaced
([ADR-038](DECISIONS.md#adr-038)). A row disappears when its session ends,
or when it drops out of a successful enumeration and has had no hook event
for a while; a failed enumeration removes nothing. A bound row can also be
hidden without disappearing: an idle, complete, or unknown session that the
VS Code extension has kept alive under a newer session in the same window
and folder is left off the list until it needs the owner or the newer
session ends ([ADR-038](DECISIONS.md#adr-038)). Exact
timing lives in [ARCHITECTURE.md](ARCHITECTURE.md#observation-channels),
since it is a daemon behaviour, not a control. There is no bind picker and no
unbind action: nothing here is managed by hand.

### Header: Hide grey, the gear, and Quit

The header carries three controls: Hide grey, a gear, and Quit, in
that order, pinned to the header's right edge
([ADR-033](DECISIONS.md#adr-033) added the gear beside the existing
Hide grey and Quit; before it, a grey toggle and Quit under
[ADR-031](DECISIONS.md#adr-031), and before that three, Hide grey,
Move, and Quit, added by [ADR-030](DECISIONS.md#adr-030) on top of
[ADR-028](DECISIONS.md#adr-028)'s original two). The whole header is
still the drag region: there is no separate grip, and dragging from any
empty part of the bar, including the count pills, moves the window.

Hide grey filters `unknown` rows out of the session list, not out of
the daemon: a single click flips `Registry.hide_unknown`
([ADR-030](DECISIONS.md#adr-030)), the daemon's own setting, persisted
and unchanged by anything in [ADR-033](DECISIONS.md#adr-033) or
[ADR-034](DECISIONS.md#adr-034). It renders as a track-and-thumb
switch, `role="switch"` and `aria-checked` on the button itself,
reading "Hide grey" while unknown rows show and "N hidden" while they
are hidden (`hideGreyWord`/`hideGreyAriaLabel` in
`app/ui/src/format.ts`, [ADR-034](DECISIONS.md#adr-034); "Hide
unknown" / "Show N unknown" before it, under
[ADR-031](DECISIONS.md#adr-031)). A hidden session is always counted,
never silently gone, and a session leaving `unknown` while hidden
still reappears on its own, moving every target below it without a
click. Because `heard` ([ADR-029](DECISIONS.md#adr-029)) resets on
every daemon restart, a session that genuinely needed the owner before
the restart also renders `unknown` until a hook fires for it again, so
hiding it can hide a row that needs a human; the hidden count is the
accepted mitigation for that, not a fix for it. See
[ADR-030](DECISIONS.md#adr-030) for the full trade-off. The control
disappears entirely, via the native `hidden` attribute, when there are
no unknown rows and hiding is already off, so it never sits there with
nothing to do; Quit's position at the header's right edge does not
move when it does.

The gear opens the settings panel in place of the session list. A
single click toggles it; the gear is an icon button,
`aria-label="Settings"`, `aria-pressed` tracking whether the panel is
open, and its own icon swaps from a cog to a back arrow when open, on
a raised, tinted background, so its pressed state is a shape and a
background change, not only a colour
([ADR-034](DECISIONS.md#adr-034); ADR-033 first added the gear and
read its pressed state from a text change, "Settings" to "Close
settings," instead). The header's state-count summary, Hide grey, and
Quit stay visible and unchanged while the panel is open; only the list
area swaps content, in the same window. See
[Settings panel](#settings-panel) below for what each row does.

Move, the click-to-place alternative to dragging the window that
[ADR-028](DECISIONS.md#adr-028) kept, is removed
([ADR-031](DECISIONS.md#adr-031)). The window is now repositioned by
dragging only, except for the panel's own Reset window position row
(see below), which moves it to a fixed default rather than letting the
owner choose where; a saved position is still restored and clamped into
the current monitors' work area at startup, but there is no click-based
way to place it at an arbitrary point afterward. This is an
owner-approved exception to [ACCESSIBILITY.md](ACCESSIBILITY.md)'s
no-required-drag rule, not a reading that satisfies it; see
ACCESSIBILITY.md for the open gap it leaves. Quit closes the daemon and
the surface together; it is icon-only, a cross glyph with
`aria-label="Quit Deckhand"` and no visible text.

### Settings panel

Four rows across two titled sections, Window and Claude Code, opened
by the gear and closed by clicking it again
([ADR-033](DECISIONS.md#adr-033); grouped into sections and cut from
five rows to four by [ADR-034](DECISIONS.md#adr-034), which combined
Hooks and Repair). Every row is a text label plus a text state, never
colour alone, whether that state reads through a toggle switch, a
status pill, or plain text:

| Section | Row | Does |
| --- | --- | --- |
| Window | Always on top | Toggles whether the window stays above every other window. Default on. Persists across restarts. A switch beside the word, not only the word. |
| Window | Start with Windows | Toggles a registry entry that launches Deckhand at login. Reads "On (other copy)" when some other Deckhand exe already owns the entry; clicking repoints it at this one rather than turning it off. A switch beside the word. |
| Window | Reset position | A button, not a toggle: moves the window to its default spot near the top-left of the current monitor and remembers that as the new saved position. Labelled "Reset position" on screen since [ADR-034](DECISIONS.md#adr-034) ("Reset window position" before it); a short description sits under the label, replaced by "Done" for a few seconds after a click. |
| Claude Code | Hooks | A status pill, tint plus text, shows whether the Claude Code hook wiring `scripts/install-hooks.ps1` installs is "Installed," "Outdated," "Missing," or "Unreadable" in `~/.claude/settings.json`. A separate Repair button beside it reruns the installer against this install's own checkout; its result shows as a short second line under the label. Styled inactive rather than natively disabled when no checkout is found nearby; the second line already reads "Installer not found" without requiring a click. Combined from two rows into one by [ADR-034](DECISIONS.md#adr-034). |

Hide grey, the header's own toggle, is not among these rows; see
[Header: Hide grey, the gear, and Quit](#header-hide-grey-the-gear-and-quit)
above for what it does.

Every switch row's state word sits to the left of its switch,
right-aligned to the switch's own left edge, so the switch itself, the
Reset row's icon, and the Repair button all end flush against the same
right edge ([ADR-034](DECISIONS.md#adr-034)).

Every command the panel calls takes no argument from the webview but
the click itself; the daemon always decides a toggle's next state,
never trusts one the surface supplies. See
[ADR-033](DECISIONS.md#adr-033) for the full record, including why
Repair and Start with Windows are the two places this surface now
writes outside its own data directory, and
[SECURITY_MODEL.md](SECURITY_MODEL.md) for the trust implications of
that.

### What has no software equivalent

| Device feature | Disposition |
| --- | --- |
| Bluetooth pairing, 3 channels | Dropped. No radio. |
| USB-C wired mode | Dropped. |
| Battery reporting | Dropped. |
| Rear power button, sleep | Dropped. Window visibility replaces it. |
| Underglow, lighting timeout (default 3 min) | Kept as an idle-dim behaviour on the surface. |
| macOS Input Monitoring permission | Not required to read status. Required only for global hotkeys, which are optional. |
| Soft reset via PCB screws | Replaced by the settings panel's Reset position row ([ADR-033](DECISIONS.md#adr-033), relabelled by [ADR-034](DECISIONS.md#adr-034)); it resets placement only, not every setting. |
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
Key, and the Codex Key. Deckhand uses **rows** for the session list,
**Hide grey** for the header's own toggle (labelled "Hide unknown" or
"Show N unknown" before [ADR-034](DECISIONS.md#adr-034) shortened its
wording), **the gear** (labelled "Settings" or "Close settings") for
the settings panel's own control, and **Quit** for closing the app.
The panel's own rows are named for what they do: Always on top, Start
with Windows, Reset window position, Hooks, and Repair. Move had no
device equivalent; it existed only in Deckhand and was removed by
[ADR-031](DECISIONS.md#adr-031). None of the device's other names apply
to anything on the current surface.
