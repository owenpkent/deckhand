<div align="center">

# 🧭 Deckhand

**A software Codex Micro for Claude Code**

An always-on-top, mouse-only surface for running several Claude Code
sessions at once: an ordered list of every session with a live status
light, name, and state word, so you can see all of them and raise any one
to the front in a single click.

  <p>
    <a href="https://github.com/owenpkent/deckhand/actions/workflows/docs.yml"><img src="https://github.com/owenpkent/deckhand/actions/workflows/docs.yml/badge.svg" alt="Docs CI"/></a>
    <img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT"/>
    <img src="https://img.shields.io/badge/status-Phase%201%3A%20observation-blue" alt="Status: Phase 1 observation"/>
    <a href="CONTRIBUTING.md"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen" alt="PRs welcome"/></a>
    <a href="https://github.com/owenpkent/deckhand/discussions"><img src="https://img.shields.io/badge/discussions-join-8A2BE2" alt="Join the discussions"/></a>
  </p>

</div>

---

**New here?** The short version is
[docs/EXECUTIVE_SUMMARY.md](docs/EXECUTIVE_SUMMARY.md), the full technical
account is [docs/WHITEPAPER.md](docs/WHITEPAPER.md), the design ledger is
[docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md), and the ways to help are in
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Status

**Phase 1: observation, started.** The specification is complete and the
first code exists: a daemon and list surface in one Tauri application
plus the hook shim, building and passing their tests, with the
observation pipeline proven end to end against live sessions.
[ADR-028](docs/DECISIONS.md#adr-028) (2026-09-13) narrowed the surface
to a session list plus Move and Quit, and replaced the six-slot binding
with an auto-binding, unbounded list; [ADR-030](docs/DECISIONS.md#adr-030),
the same day, added a third header control, Hide grey, that filters
`unknown` rows out of the list and shows how many it hid on the button
itself. [ADR-031](docs/DECISIONS.md#adr-031), also the same day, then
removed Move (the window is now repositioned by dragging only, an
owner-approved exception recorded in
[docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md)) and left the header
holding a grey toggle and Quit. [ADR-033](docs/DECISIONS.md#adr-033)
(2026-09-14) then added a gear button, beside the existing grey
toggle, that opens an in-bar settings panel in place of the session
list: always on top, start with Windows, reset window position, and a
hooks-installed status with a Repair action. The code implements all
of these, and on its first run bound ten live sessions across four
repos. [ADR-034](docs/DECISIONS.md#adr-034)
(2026-09-14) then redrew the header, panel, and session rows: icon
buttons for the gear and Quit, toggle switches beside their On/Off
words (the header's own grey toggle included, relabelled "Hide grey" /
"N hidden"), a combined Hooks and Repair row, the panel grouped into
two titled sections, rounded rows with a left accent bar, a shared
horizontal inset for rows and panel cards, and a window-sizing fix for
a scrollbar that used to appear on the open panel. One Phase 0 item
also stays open alongside it: hook payload validation against a live
install has ten of the twelve documented events observed, with
`Notification` and `StopFailure` still unseen.
[ADR-035](docs/DECISIONS.md#adr-035) (2026-09-15) then gave the daemon
a second liveness channel: a process handle held per session, polled
on the existing two-second tick, so a crash without `SessionEnd` moves
a session straight to `ended`, and the periodic `claude agents --json`
scan now colours a session no hook has coloured yet, narrowing
`T_unknown` for a session with a live handle.
[ADR-036](docs/DECISIONS.md#adr-036), the same day, let the scan break
a tie with a hook-set colour after two consecutive contradicting
scans, and retired the planned transcript JSONL fallback before it was
built. [ADR-037](docs/DECISIONS.md#adr-037), also that day, closed the
daemon's process lifecycle: a named mutex makes it single instance,
and a same-binary watchdog relaunches it after a crash, rate-limited
by a local, append-only ledger; Start with Windows is unchanged.

| Piece | State |
| --- | --- |
| Control mapping (device to software) | ✅ Written |
| Architecture and adapter contract | ✅ Written |
| Claude Code adapter design | ✅ Written, ⏳ partially verified against 2.1.220 |
| Security model for permission gating | ✅ Written |
| UI and accessibility specification | ✅ Written, narrowed by ADR-028 |
| Tauri no-focus-steal window spike | ✅ Passed on Windows 11 (ADR-025) |
| Hook payload validation spike | ⏳ Ten of twelve events observed live; two remain |
| Daemon, shim, state machine | ✅ Phase 1 skeleton; live sessions paint real status |
| Session-list surface, auto-binding | ✅ Built (ADR-028); binds every enumerated session, prunes ended ones |
| Settings panel (always on top, start with Windows, reset position, hooks status, Repair) | ✅ Built (ADR-033), regrouped and restyled (ADR-034); Start with Windows and Repair unverified against a real login and a real settings.json |
| Session liveness (process handle, scan status, tie-break) | ✅ Built (ADR-035, ADR-036); `shell` and `waiting` scan statuses still unobserved live |
| Process lifecycle (single instance, crash watchdog) | ✅ Built (ADR-037); Start with Windows unchanged; the watchdog ledger is not yet surfaced in the settings panel |
| Approve and deny | ❌ Phase 2, nothing has write authority yet, and not currently planned on the surface |

Build and run it with `python run.py`, which checks the toolchain,
compiles the TypeScript surface, builds the Rust workspace, and restarts
the app (`--test` to run the suites first, `--no-build` to just restart,
`--check` to report the toolchain, `--stop` to stop it).
`scripts/build-app.ps1` is the build-only equivalent, and it runs the
tests: `cargo test --workspace` for the daemon and shim, `npm test` in
`app/ui` for the surface.

---

## Why Deckhand?

Running several Claude Code sessions at once turns you into a human poller:
alt-tab, read, alt-tab, read. The cost falls hardest on people for whom every
window switch is expensive. The author is a wheelchair user with muscular
dystrophy; moving a pointer is cheap, pressing keys is not, and checking six
terminals by keyboard is exactly the tax this project removes.

Deckhand is designed as a **status board first**: a list you can read at a
glance, using the Codex Micro's colour language. White idle, blue thinking,
green done-and-unread, amber waiting on you, red problem. As of
[ADR-028](docs/DECISIONS.md#adr-028), that list and a single click to raise
a session's window are currently the whole surface. Acting on a session,
approving a call, denying one, or answering a question, stays Phase 2 or
later work, and is no longer planned to land on this surface as designed;
a returning control needs its own ADR.

Everything is operable with a pointer alone. Keyboard and voice are
conveniences, never requirements. That rule is load-bearing and
non-negotiable; see [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md).

---

## Inspiration

The [Codex Micro](https://learn.chatgpt.com/docs/features/codex-micro) is a
limited-run macropad by [Work Louder](https://worklouder.cc/) and OpenAI: six
agent keys with status LEDs, six command keys, a stick, a dial, push-to-talk.
It is a genuinely good piece of interaction design, and Deckhand copies its
model deliberately and credits it plainly. What the hardware cannot do is be
free, be available after the run ends, drive Claude Code, or be usable without
functioning hands. Those four gaps are the project.

Where Deckhand diverges from the device, it says so and says why:
[docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md#deliberate-divergences).

---

## What it will do

[ADR-028](docs/DECISIONS.md#adr-028) narrowed this to the two things below.
Everything else the device does (command keys, a stick, a dial, push-to-talk,
a touch sensor for layers and pairing) has no current Deckhand equivalent;
see [docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md#removed-from-the-surface)
for what was planned and why it was cut.

| Control | On the device | In Deckhand |
| --- | --- | --- |
| 6 agent keys | One chat each, LED status | One row per Claude Code session, in an unbounded list, live status |
| Press a key | Switch chat, or double-press to raise it | Click a row: select the session and raise its window, in one click |

## What it will look like

These are **design mockups**, not screenshots, and they predate
[ADR-028](docs/DECISIONS.md#adr-028) (2026-09-13), which narrowed the
target to a vertical session list with Move and Quit in the header,
[ADR-030](docs/DECISIONS.md#adr-030), the same day, which added a third
header control, Hide grey, [ADR-031](docs/DECISIONS.md#adr-031), also
the same day, which removed Move and left the header holding a grey
toggle and Quit, [ADR-033](docs/DECISIONS.md#adr-033) (2026-09-14),
which added a gear button beside the grey toggle that opens a settings
panel, and [ADR-034](docs/DECISIONS.md#adr-034), the next day, which
redrew that gear as an icon, restyled the header, panel, and session
rows, and fixed the panel's window sizing. The mockups below still show the
wider control set from before all five changes: six tiles in a
horizontal strip, command keys, a stick, a dial, talk and send. None
of that is the current design. Where
a mockup and
[docs/UI_SPEC.md](docs/UI_SPEC.md) disagree, the spec wins; treat the
images as historical until they are redrawn.

<img src="assets/surface-horizontal.svg" width="100%" alt="Mockup of the
Deckhand surface: a dark horizontal panel. Left, six square tiles: undertow with
a white idle ring, contour with a blue thinking ring and the subtitle Bash
cmake, deckhand with a thick amber ring, a hand glyph, subtitle Bash approval,
and a selection chevron, meshview with a green ring, a check and an unread dot,
markcopy grey and hatched with a question mark and the words state unknown, and
an empty dashed tile reading bind a session. Middle, six command keys: Approve
and Deny enabled, Continue and Interrupt greyed out, Plan and Compact neutral.
Right, a four-way arrow pad, a dial reading Opus, model, with minus and plus
targets, and Talk and Send buttons. Bottom left, three layer dots labelled Layer
1: Claude Code."/>

*The surface, horizontal, docked to a screen edge. One tile per session; the
amber tile is selected, so Approve and Deny are live and everything that does
not apply is disabled rather than hidden.*

<img src="assets/states.svg" width="100%" alt="Legend of the seven tile states,
each a small tile with its own colour, glyph, and words: Idle, white with an
open circle and alive, waiting. Thinking, blue with an arc spinner and working.
Needs input, amber with a hand glyph and waiting on you. Complete, green with a
check, an unread dot, and done, unread, clears on select. Error, red with a
cross and crashed or failed. Unknown, grey and hatched with a question mark and
never guessed. Ended or unbound, a dashed outline with a plus and empty."/>

*The colour language, inherited from the Codex Micro. Every state also has a
glyph and a label; colour is never the only channel.*

<p align="center"><img src="assets/detail-approval.svg" width="60%" alt="Mockup
of the detail panel during a pending approval. Header: Tile 3, deckhand, Opus,
attached. An amber pill reads WAITING ON YOU. Below, the text Claude Code wants
to run, then a monospace card showing Bash and the command cmake dash dash build
build dash dash config Release. Underneath, a green Approve button and a
red-outlined Deny button separated by a wide gap, with a note reading answers in
52 seconds, then returns to the terminal. At the bottom, a context bar at 62
percent, 41 cents this session, and Raise window, Unbind, and Settings
buttons."/></p>

*The approval card. The tool input is shown before the buttons are live, Deny
sits a full dead gap away from Approve, and if you do nothing the decision
returns safely to the terminal.*

## How it will work

Claude Code fires hooks. A tiny shim forwards each hook's JSON to a local
daemon, which runs one state machine per session and drives the surface.
When Phase 2 wires up permission gating, the `PreToolUse` hook will be held
open while the row burns amber, and a click will travel back as a
documented `permissionDecision`; exactly what that click looks like is
undecided, since [ADR-028](docs/DECISIONS.md#adr-028) removed the command
keys it was going to be. If Deckhand cannot answer in time, it answers
`ask` and Claude Code prompts you normally: every failure path returns the
decision to you, none of them auto-allow.

Two modes per session:

- **Attached**: you started the session in your terminal. Full status, full
  approve and deny. No prompt injection: the channels Claude Code documents
  all deliver at a turn boundary, never into an idle session, and none has
  been observed working here, so Deckhand ships no send rather than faking
  one.
- **Hosted** (later): Deckhand starts the session via the Claude Agent SDK and
  can do everything, at the cost of being the session's only UI.

Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md).

## Project structure

```
deckhand/
├── docs/                  The specification (start here)
│   ├── EXECUTIVE_SUMMARY.md   Two pages, no jargon
│   ├── WHITEPAPER.md          The whole system in one technical paper
│   ├── CONTROL_MAPPING.md     Device control to software control, with reasons
│   ├── ARCHITECTURE.md        Daemon, shim, surface, state machine
│   ├── ADAPTER_PROTOCOL.md    The contract any agent runtime plugs into
│   ├── CLAUDE_CODE_ADAPTER.md The reference adapter, stability-annotated
│   ├── SECURITY_MODEL.md      What the approve button must never do
│   ├── UI_SPEC.md             Session list, header, themes
│   ├── ACCESSIBILITY.md       The rules everything else answers to
│   ├── DECISIONS.md           ADRs: what was decided and why
│   └── WORKFLOW.md            Source-of-truth map, change propagation
├── app/                   The Phase 1 application: Rust daemon plus
│                          TypeScript surface, one Tauri window
├── shim/                  The tiny program Claude Code's hooks call
├── spikes/                Frozen Phase 0 evidence (ADR-025)
├── scripts/               Build, run, and docs-gate tooling
├── .github/               CI, issue and PR templates
└── ...
```

## Getting started

To read, start with the executive summary, then the control mapping. To
run the Phase 1 board (Windows, with Rust, Node, and Python installed):

```powershell
git clone https://github.com/owenpkent/deckhand.git
cd deckhand
python run.py
```

### Registering the shim

A repo's sessions only report to the daemon once the shim is wired into
a Claude Code hook. Wire it once, at the user level, and every repo's
sessions are covered:

```powershell
powershell -NoProfile -File scripts\install-hooks.ps1
```

The script merges an entry for each of the twelve hook events into
`%USERPROFILE%\.claude\settings.json`, backs the file up first, and is
safe to run again: entries already installed are left alone. To remove
everything it added:

```powershell
powershell -NoProfile -File scripts\install-hooks.ps1 -Uninstall
```

The most valuable contribution is still challenging the spec:
[CONTRIBUTING.md](CONTRIBUTING.md) lists concrete starting points.

## Accessibility

Deckhand is an accessibility project wearing a productivity tool's clothes.
The hard rule: every feature fully operable with single clicks on stationary
targets. No required holds, drags, double-clicks, hovers, or keys. Minimum
44 px hit targets, 100 to 300% scaling, and status that never relies on
colour alone. Feedback from disabled users outranks every estimate in this
repository:
[accessibility feedback template](https://github.com/owenpkent/deckhand/issues/new?template=accessibility_feedback.yml).

## Related projects

| Project | What it is | Relation |
| --- | --- | --- |
| [alpha-osk](https://github.com/owenpkent/alpha-osk) | Mouse-only on-screen keyboard | Sibling; solved the no-focus-steal window Deckhand must reproduce |
| [alpha-stick](https://github.com/owenpkent/alpha-stick) | Adaptive gaming joystick | Sibling; this repo mirrors its documentation discipline |
| MacroVox | Voice to clipboard | Sibling; Deckhand's talk button delegates to it |
| Nimbus | Adaptive virtual joystick | Sibling; future pointer source for switch users |
| [Codex Micro](https://learn.chatgpt.com/docs/features/codex-micro) | The hardware original | Credited inspiration, not affiliated |

Deckhand is not affiliated with, or endorsed by, OpenAI, Work Louder, or
Anthropic. It is an independent tool that observes and drives Claude Code
through its documented extension points.

## Roadmap

Phase 0 specification → 1 observation-only session list (now) → 2 approve
and deny → 3 and 5 retired by ADR-028 → 4 hosted mode → 6 a second
adapter. Details and exit criteria: [ROADMAP.md](ROADMAP.md).

## Contributing

Spec review, accessibility feedback, and attempts to break the security model
are the contributions that matter most right now. See
[CONTRIBUTING.md](CONTRIBUTING.md) and the
[discussions](https://github.com/owenpkent/deckhand/discussions).

## License

MIT. See [LICENSE](LICENSE).

## Acknowledgments

- **Work Louder and OpenAI**, for the Codex Micro's interaction design, which
  this project studies and reimplements in software with respect.
- **Anthropic**, for shipping the hooks and permission interfaces that make an
  honest external control surface possible at all.

---

<div align="center">

**Every session, one glance, zero keys.**

</div>
