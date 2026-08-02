<div align="center">

# 🧭 Deckhand

**A software Codex Micro for Claude Code**

An always-on-top, mouse-only control surface for running several Claude Code
sessions at once: six tiles with live status lights so you can see every
session, answer the questions it asks you, and approve the calls that need a
human, plus the rest of the macropad reimagined for a pointer.

  <p>
    <a href="https://github.com/owenpkent/deckhand/actions/workflows/docs.yml"><img src="https://github.com/owenpkent/deckhand/actions/workflows/docs.yml/badge.svg" alt="Docs CI"/></a>
    <img src="https://img.shields.io/badge/license-MIT-blue" alt="License: MIT"/>
    <img src="https://img.shields.io/badge/status-Phase%200%3A%20specification-orange" alt="Status: Phase 0 specification"/>
    <a href="CONTRIBUTING.md"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen" alt="PRs welcome"/></a>
    <a href="https://github.com/owenpkent/deckhand/discussions"><img src="https://img.shields.io/badge/discussions-join-8A2BE2" alt="Join the discussions"/></a>
  </p>

</div>

---

**New here?** The short version is
[docs/EXECUTIVE_SUMMARY.md](docs/EXECUTIVE_SUMMARY.md), the design ledger is
[docs/CONTROL_MAPPING.md](docs/CONTROL_MAPPING.md), and the ways to help are in
[CONTRIBUTING.md](CONTRIBUTING.md).

---

## Status

**Phase 0: specification.** There is no application yet. What exists is a
complete, reviewable design plus the first of the two de-risking spikes:
every control mapped from the original device, the architecture, the
adapter contract, the security model for the approve button, the
accessibility rules the rest must obey, and a working proof that a Tauri
window on Windows can be always-on-top without stealing focus.

| Piece | State |
| --- | --- |
| Control mapping (device to software) | ✅ Written |
| Architecture and adapter contract | ✅ Written |
| Claude Code adapter design | ✅ Written, ⏳ partially verified against 2.1.220 |
| Security model for permission gating | ✅ Written |
| UI and accessibility specification | ✅ Written |
| Tauri no-focus-steal window spike | ✅ Passed on Windows 11 (ADR-025) |
| Hook payload validation spike | ⏳ `PreToolUse` fully enumerated, other events open |
| The application itself | ❌ Not yet, Phase 1 |

---

## Why Deckhand?

Running several Claude Code sessions at once turns you into a human poller:
alt-tab, read, alt-tab, read. The cost falls hardest on people for whom every
window switch is expensive. The author is a wheelchair user with muscular
dystrophy; moving a pointer is cheap, pressing keys is not, and checking six
terminals by keyboard is exactly the tax this project removes.

Deckhand is designed as a **status board first**: six tiles you can read at a
glance, using the Codex Micro's colour language. White idle, blue thinking,
green done-and-unread, amber waiting on you, red problem. Amber is usually a
question, not a permission prompt: on one machine running Claude Code with
`permissions.defaultMode: "auto"`, the classifier answers most permission
requests on its own, and the human is asked to pick an option far more often
than asked to allow a tool, 322 `AskUserQuestion` calls across 155 of 240
sessions, against 10 to 27 tool denials in the same corpus. That is one
user's habits on one machine, not a claim about how everyone works, but it is
why Deckhand leads with **see every session, answer its questions, and
approve the calls that need a human**, rather than with approve and deny
alone. Approve and Deny stay one click on the surface, without touching the
terminal; they are simply not the whole story anymore.

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

| Control | On the device | In Deckhand |
| --- | --- | --- |
| 6 agent keys | One chat each, LED status | One Claude Code session each, live status tile |
| 6 command keys | Approve, decline, continue, send... | Approve, Deny, Answer, Interrupt, Continue, Reveal |
| Stick | Plan mode, history, sidebar | Scroll the detail panel, expand it, return to the last tile |
| Dial | Composer options, reasoning default | Session options: model, effort, permission mode |
| Mic key | Push-to-talk | Click-to-toggle talk, delegating speech to MacroVox |
| Codex key | Send | Send (hosted mode; honest about attached mode) |
| Touch sensor | Layers, pairing | Layer strip, no pairing to do |

## What it will look like

These are **design mockups drawn from [docs/UI_SPEC.md](docs/UI_SPEC.md)**,
not screenshots. Nothing runs yet; this is the target the spec commits to.
They also predate the current command key set: the drawings still show Plan
and Compact where the spec now has Answer and Reveal, and a "Raise window"
button where it now says Reveal. Where a mockup and `docs/UI_SPEC.md`
disagree, the spec wins and the drawing is the thing that is out of date.

<img src="assets/surface-horizontal.svg" width="100%" alt="Mockup of the Deckhand surface: a dark horizontal panel. Left, six square tiles: undertow with a white idle ring, contour with a blue thinking ring and the subtitle Bash cmake, deckhand with a thick amber ring, a hand glyph, subtitle Bash approval, and a selection chevron, meshview with a green ring, a check and an unread dot, markcopy grey and hatched with a question mark and the words state unknown, and an empty dashed tile reading bind a session. Middle, six command keys: Approve and Deny enabled, Continue and Interrupt greyed out, Plan and Compact neutral. Right, a four-way arrow pad, a dial reading Opus, model, with minus and plus targets, and Talk and Send buttons. Bottom left, three layer dots labelled Layer 1: Claude Code."/>

*The surface, horizontal, docked to a screen edge. One tile per session; the
amber tile is selected, so Approve and Deny are live and everything that does
not apply is disabled rather than hidden.*

<img src="assets/states.svg" width="100%" alt="Legend of the seven tile states, each a small tile with its own colour, glyph, and words: Idle, white with an open circle and alive, waiting. Thinking, blue with an arc spinner and working. Needs input, amber with a hand glyph and waiting on you. Complete, green with a check, an unread dot, and done, unread, clears on select. Error, red with a cross and crashed or failed. Unknown, grey and hatched with a question mark and never guessed. Ended or unbound, a dashed outline with a plus and empty."/>

*The colour language, inherited from the Codex Micro. Every state also has a
glyph and a label; colour is never the only channel.*

<p align="center"><img src="assets/detail-approval.svg" width="60%" alt="Mockup of the detail panel during a pending approval. Header: Tile 3, deckhand, Opus, attached. An amber pill reads WAITING ON YOU. Below, the text Claude Code wants to run, then a monospace card showing Bash and the command cmake dash dash build build dash dash config Release. Underneath, a green Approve button and a red-outlined Deny button separated by a wide gap, with a note reading answers in 52 seconds, then returns to the terminal. At the bottom, a context bar at 62 percent, 41 cents this session, and Raise window, Unbind, and Settings buttons."/></p>

*The approval card. The tool input is shown before the buttons are live, Deny
sits a full dead gap away from Approve, and if you do nothing the decision
returns safely to the terminal.*

## How it will work

Claude Code fires hooks. A tiny shim will forward each hook's JSON to a local
daemon, which will run one state machine per session and drive the tiles. When
permission gating is on, the `PreToolUse` hook is held open while the tile
burns amber; your click travels back as a documented
`permissionDecision`. If Deckhand cannot answer in time, it answers `ask` and
Claude Code prompts you normally: every failure path returns the decision to
you, none of them auto-allow.

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
│   ├── CONTROL_MAPPING.md     Device control to software control, with reasons
│   ├── ARCHITECTURE.md        Daemon, shim, surface, state machine
│   ├── ADAPTER_PROTOCOL.md    The contract any agent runtime plugs into
│   ├── CLAUDE_CODE_ADAPTER.md The reference adapter, stability-annotated
│   ├── SECURITY_MODEL.md      What the approve button must never do
│   ├── UI_SPEC.md             Tiles, keys, dial, stick, themes
│   ├── ACCESSIBILITY.md       The rules everything else answers to
│   ├── DECISIONS.md           ADRs: what was decided and why
│   └── WORKFLOW.md            Source-of-truth map, change propagation
├── .github/               CI, issue and PR templates
└── (no code yet: that is Phase 1)
```

## Getting started

There is nothing to install yet. To read or contribute:

```powershell
git clone https://github.com/owenpkent/deckhand.git
cd deckhand
```

Start with the executive summary, then the control mapping. If you want to
help before code exists, the most valuable work is challenging the spec:
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

Phase 0 specification (now) → 1 observation-only tiles → 2 approve and deny →
3 the full surface → 4 hosted mode → 5 talk → 6 a second adapter. Details and
exit criteria: [ROADMAP.md](ROADMAP.md).

## Contributing

Spec review, accessibility feedback, and attempts to break the security model
are the Phase 0 contributions that matter most. See
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

**Six agents, one glance, zero keys.**

</div>
