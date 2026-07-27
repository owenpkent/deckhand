# Architecture

Status: **proposed**. Nothing here is built. This document exists so the first
line of code has somewhere to go.

## The one-paragraph version

A long-lived local daemon holds all the state. Claude Code is configured to fire
hooks that report into that daemon over a loopback socket. The daemon keeps a
state machine per session and pushes changes to a Tauri window that draws the
surface. When you click Approve, the answer travels back out through the hook
that is still blocked and waiting for it.

## Processes

```
                        loopback HTTP, 127.0.0.1
  claude (session 1) ──► hook shim ──┐
  claude (session 2) ──► hook shim ──┤
  claude (session 3) ──► hook shim ──┼──► deckhand daemon ──► Tauri window
  ...                                │      (Rust)              (TypeScript)
  claude (session 6) ──► hook shim ──┘         │                     ▲
                                               └── IPC / events ─────┘
```

| Process | Language | Lifetime | Responsibility |
| --- | --- | --- | --- |
| Hook shim | Small native binary or script | Milliseconds, one per hook fire | Read hook JSON on stdin, POST it to the daemon, write the daemon's answer to stdout |
| Daemon | Rust | Runs as long as the surface is open | Session registry, state machines, pending-approval queue, adapter host, settings |
| Surface | TypeScript in a Tauri webview | Same as daemon | Draw tiles, take pointer input, nothing else |

The daemon and the surface ship in one Tauri application. They are described
separately because the daemon must keep working while the window is hidden, and
because a future headless or remote surface should be able to attach to the same
daemon.

### Why a separate hook shim

Hooks fire as short-lived subprocesses, potentially on every tool call across
six sessions. Whatever they run has to start fast. The shim does one thing:
forward stdin, return stdout. It holds no state and knows nothing about
Claude Code semantics. All interpretation happens in the daemon.

Process spawn cost is the main performance risk in this design. See
[open questions](#open-questions).

## The session state machine

One instance per bound session. This is the only place a status colour is
decided.

```
                    ┌──────────────────────────────────────┐
                    ▼                                      │
  (unbound) ──► IDLE ──► THINKING ──► COMPLETE ────────────┘
     ▲            ▲          │             │
     │            │          ▼             │
     │            └──── NEEDS_INPUT ◄──────┘
     │                       │
     └──── ENDED ◄──── ERROR ┘
```

| State | Tile | Entered when | Left when |
| --- | --- | --- | --- |
| `IDLE` | White | Session starts, or you select a `COMPLETE` tile | A turn begins |
| `THINKING` | Blue | A turn begins, or a tool starts | A turn ends, or input is required |
| `NEEDS_INPUT` | Amber | A permission decision is pending, or Claude asked a question | The decision is made, or the question is answered |
| `COMPLETE` | Green | A turn finished and you have not selected the tile since | You select the tile |
| `ERROR` | Red | The turn failed, the process died, or the session went silent past its deadline | You select the tile, or the session recovers |
| `ENDED` | Off | The session exited | Rebound |
| `UNKNOWN` | Grey, hatched | The daemon cannot currently tell | Any authoritative event arrives |

`UNKNOWN` is deliberate and load-bearing. A status board that guesses is worse
than one that admits it does not know, because the whole value is being able to
trust a glance. Any time the daemon loses its footing, for example after a
restart with sessions already running, tiles go grey rather than assuming idle.

### Liveness

Events are the primary signal, but absence of events is ambiguous: a session
thinking hard and a session whose terminal was closed both emit nothing. The
daemon therefore holds a per-session deadline. A session in `THINKING` with no
event for longer than the deadline moves to `UNKNOWN`, not `ERROR`, because a
long tool call is normal. Confirmed process death moves it to `ENDED`.

The deadline default and how process death is confirmed on Windows are both
unresolved. See [open questions](#open-questions).

## Modes

The mode is a property of a session, not of the application. A single surface
can show attached sessions and hosted sessions side by side, and the tile should
make clear which is which.

### Attached mode

Deckhand watches a `claude` session that you started in your own terminal.

- Status observation: **full**, through hooks.
- Approve and deny: **full**, through the `PreToolUse` hook.
- Send, continue, interrupt: **not available through any supported interface.**
  An opt-in fallback synthesises keystrokes at the session's terminal window.
  It is off by default and clearly marked as unreliable.

### Hosted mode

Deckhand starts and owns the session through the Claude Agent SDK.

- Everything works, including sending prompts.
- The cost is that the session has no terminal UI of its own. The detail panel
  becomes the only place to read the transcript, which is a significant amount
  of surface Deckhand would have to build well.

Attached mode is built first because it is the one that improves a workflow that
already exists. Hosted mode is Phase 4.

## The adapter boundary

The daemon knows nothing about Claude Code. It talks to adapters, which
implement the contract in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md). The
Claude Code adapter is the reference implementation and is documented in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

This boundary is not speculative generality. It exists because the Claude Code
integration deliberately mixes documented interfaces with fragile ones, and the
boundary is where that risk gets contained: when an internal changes, one
adapter breaks, and the surface, the state machine, and the settings do not.

## Transport

Between hook shim and daemon: **loopback HTTP on 127.0.0.1**, with a token.

Considered and rejected:

| Option | Why not |
| --- | --- |
| Named pipes / Unix sockets | Better isolation, but three platform implementations and a fiddly permissions story. Revisit if the token proves inadequate. |
| A shared state file | No way to block on an approval, which is the whole point. |
| stdin/stdout to a long-lived child | Hooks are independent subprocesses, so there is nothing to keep alive. |

Loopback is not a security boundary on a multi-user machine. Every local process
can reach it. The token stops accidents, not a determined local attacker. This
matters because the endpoint can approve tool calls, so it is treated seriously
in [SECURITY_MODEL.md](SECURITY_MODEL.md).

## The approval path

This is the most important flow in the system and the one most worth getting
right.

```
 1. Claude decides to run a tool
 2. PreToolUse hook fires, shim POSTs the tool name and input, then blocks
 3. Daemon creates a pending approval, moves the session to NEEDS_INPUT
 4. Tile goes amber; Approve and Deny become enabled
 5. You click. Or a rule decides. Or the timeout expires
 6. Daemon answers the still-open request
 7. Shim writes the permission decision to stdout and exits
 8. Claude Code honours it
```

Step 5 has three ways out and all three must be designed, because a hook that
never returns stalls a session:

- **You decide.** The normal path.
- **A rule decides.** Optional, off by default, and it must be legible: the tile
  shows that a rule answered and which one.
- **The timeout expires.** The daemon must answer before the hook's own timeout,
  and it must **fail closed**. If Deckhand cannot get an answer, the safe
  default is to hand the decision back rather than to allow. See
  [SECURITY_MODEL.md](SECURITY_MODEL.md).

If the daemon is not running at all, the shim must fail in the direction that
leaves Claude Code working normally rather than blocking every tool call
forever. That behaviour is a correctness requirement, not a nicety.

## The surface

The webview draws and takes pointer input. It holds no authority and no
inference. It receives state and sends intents.

Two window properties are hard requirements rather than preferences:

1. **Always on top.**
2. **Never takes focus.** Clicking a tile must not defocus the terminal you are
   controlling. On Windows the Tauri and Qt-level flags are not sufficient on
   their own: the sibling project `alpha-osk` had to apply
   `WS_EX_NOACTIVATE | WS_EX_TOPMOST` through a raw `SetWindowLongW` call, and
   reapply it whenever the window becomes visible. Deckhand has to reproduce
   that in Tauri.

Prototyping this in Tauri before anything else is built is a Phase 0 task,
because if it cannot be made to work the stack choice is wrong and it is much
cheaper to learn that now. See [DECISIONS.md](DECISIONS.md#adr-002).

## Stack

Tauri v2, Rust daemon, TypeScript frontend. Recorded with its alternatives and
its risks in [DECISIONS.md](DECISIONS.md#adr-002).

## Persistence

| Data | Where | Notes |
| --- | --- | --- |
| Settings, layers, bindings | Local config directory, JSON | Portable, hand-editable |
| Tile-to-session bindings | Same | Bindings are by session id, which survives restarts |
| Approval audit log | Local, append-only, optional | Off by default. If Deckhand approves tool calls, being able to answer "what did I approve" is worth having |
| Session transcripts | Not stored | Deckhand reads them where they already are and copies nothing |

Nothing leaves the machine. There is no telemetry, no account, and no network
egress other than loopback.

## Open questions

These are real and unresolved. They are tracked in [TODO.md](../TODO.md).

1. **Hook overhead at six concurrent sessions.** A subprocess per tool call
   across six busy sessions could be noticeable. Needs measuring before the
   design is trusted. If it is too slow, the fallback is to hook only the events
   needed for status and gate permissions on a narrower matcher.
2. **Whether `ERROR` is detectable at all.** Amber and blue and green are
   straightforward. A failed turn may not surface as a distinct hook event. If
   it does not, red may only ever mean "the process died", and the spec should
   say so honestly rather than promise a colour that never lights.
3. **Confirming process death on Windows** cheaply enough to poll.
4. **Whether the terminal keystroke fallback is worth shipping at all.** It may
   be that attached mode should simply not offer send, and that wanting to send
   is the reason to use hosted mode.
5. **Whether one daemon should serve several surfaces**, for example a second
   window on a tablet.
6. **Binding stability across `--resume`.** Resuming appears to continue under
   the same session id, but a tile pointing at a session that forked needs
   defined behaviour.
