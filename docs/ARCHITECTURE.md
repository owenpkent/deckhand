# Architecture

Status: **accepted**. The Phase 1 skeleton, `app/` for the daemon and
surface and `shim/` for the hook shim, implements this document's
observation half; the approval path is designed here and not yet built
(Phase 2). Where the code and this file disagree, this file wins and the
code is the bug.

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
| `THINKING` | Blue | A turn begins, or an operation opens | The turn ends with nothing open, or input is required |
| `NEEDS_INPUT` | Amber | A permission decision is pending, or the session asked a question | The decision is made, or the question is answered |
| `COMPLETE` | Green | A turn finished, the child ledger is empty, and you have not selected the tile since | You select the tile, or a new turn begins |
| `ERROR` | Red | The turn failed, or the process died without a clean exit | You select the tile (a crashed session then shows `ENDED`), or the session recovers |
| `ENDED` | Off | The session exited for good, or you acknowledged a crashed `ERROR` tile | Rebound |
| `UNKNOWN` | Grey, hatched | The daemon cannot currently tell | Any authoritative event arrives |

Amber carries a kind, `permission` or `question`, on the update that raises it.
This is a discriminator on the state's detail, not a new state and not a new
colour: amber is still amber. No control on the current surface reads it, so
it does not change how a row looks; a permission control and a question
control would each key off it if either returns, per
[ADR-028](DECISIONS.md#adr-028).
See [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types) and
[DECISIONS.md](DECISIONS.md#adr-013).

Green means finished and unread. It clears when you select the tile, and it is
also left when a new turn begins, because the session is no longer finished.
The daemon records `unreadSince`, the moment the tile went green. Unread
stays a colour and never becomes a badge, per
[ACCESSIBILITY.md](ACCESSIBILITY.md#the-economics) and
[DECISIONS.md](DECISIONS.md#adr-008).

`UNKNOWN` is deliberate and load-bearing. A status board that guesses is worse
than one that admits it does not know, because the whole value is being able to
trust a glance. Any time the daemon loses its footing, for example after a
restart with sessions already running, tiles go grey rather than assuming idle.

### Events that must not be taken at face value

Two lifecycle events read like state changes and are not. Both are frequent,
and either one taken literally paints a wrong colour on a tile someone is
watching.

| Event | The naive mapping | What the daemon does |
| --- | --- | --- |
| A session-start event whose source is a compaction | `IDLE` | Nothing. It fires mid-turn, so the naive mapping flips a live blue tile white |
| A session-end event whose reason is a clear or a resume | `ENDED` | Nothing. Each is followed by a new session start, under a session that never stopped |

Both splits are `documented` for Claude Code and unverified against a live
install. The field names are in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md#status-inference). The rule
generalises past one runtime: an adapter reports a lifecycle change only when
the lifecycle actually changed, and the daemon takes no state change from an
event whose reason it does not recognise.

### The child ledger

A turn can finish while work it started is still running. On the owner's
corpus roughly one turn in ten ended with children still live, which is one
machine and one user, so read it as indicative rather than general. A tile that
goes green there says the session is done when it is not, and that falsifies
the one promise the board makes.

Each session holds a ledger of its open children. Entries are `kind:
"subagent"` only: `SubagentStart` adds one, `SubagentStop` removes it.

- `COMPLETE` is unreachable while the ledger is non-empty. A turn that ends
  with children live stays `THINKING`.
- The count is internal bookkeeping for that gate. It is not currently shown
  anywhere in the UI: corner badges are removed
  ([ADR-028](DECISIONS.md#adr-028)).
- Background Bash tasks emit no hook, so the ledger cannot see them and the
  count does not include them. That is stated plainly because a count which
  silently undercounts is worse than no count at all.
- No per-child list, no per-child approval targets, no subagent layer.

Recorded in [DECISIONS.md](DECISIONS.md#adr-019).

### Liveness, by open operation

Events are the primary signal, but absence of events is ambiguous: a session
thinking hard and a session whose terminal was closed both emit nothing. Turn
duration cannot separate them. Measured on the owner's corpus, p90 turn
duration is 660 s and p99 is over 40 minutes, so a turn-duration deadline set
anywhere useful greys healthy sessions.

The daemon brackets operations instead. An operation is open from the first
event below until its partner arrives:

| Opened by | Closed by |
| --- | --- |
| `PreToolUse` | The matching `PostToolUse` or `PostToolUseFailure` |
| `SubagentStart` | The matching `SubagentStop` |

While any operation is open the session stays `THINKING`, and the tile shows
elapsed-in-operation rather than elapsed-in-state, because "this tool call has
been running four minutes" is the number a person can act on.

`Task*` events are not in that table. They are teammate-task hooks rather than
the `/tasks` ones, and they bracket nothing the daemon models.

A `PreToolUse` can also end without a post-tool event, and each way has to be
handled or the bracket leaks. Bracketing is the daemon's rule, not the
adapter's: an adapter reports the events, this file decides what they close.
[ADR-016](DECISIONS.md#adr-016) says "the adapter defines what closes a
`PreToolUse` that ends in denial or interrupt", which reads as the opposite;
what it means is that the adapter has to report those ends at all, which is
the obligation [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types) states. The
three cases:

- **Denied.** No tool runs, so nothing follows. The bracket closes when
  Deckhand answers `deny`, and when Deckhand observes the runtime denying the
  call itself.
- **Interrupted.** An interrupt closes every operation open on that session.
  Which events actually fire on an interrupt is unverified, so `T_unknown`
  below is the backstop if the runtime disagrees.
- **Handed back with `ask`.** The bracket stays open, because the call may
  still run once the runtime's own prompt is answered. It closes on the
  post-tool event, on an observed denial, or on `T_unknown`.

One deadline, not two:

- **`T_unknown`, default 900 s**, measured from the last event of any kind.
  On expiry the session moves to `UNKNOWN`, never to `ERROR`, because a long
  tool call is normal and a wrong red costs more than an honest grey.
- The stale clock, meaning "nothing has been heard for a while", is suspended
  while an operation is open. `T_unknown` is not suspended by anything.

The asymmetry is the whole point. A terminal killed mid-tool-call leaves an
operation open that nothing will ever close, so letting an open operation
suspend `T_unknown` as well would pin that tile blue until the daemon
restarted. There is no second stale tier and no stale badge: one deadline, one
grey. Recorded in [DECISIONS.md](DECISIONS.md#adr-016).

A clean exit moves the session to `ENDED`; confirmed process death without one
is a crash and moves it to `ERROR`. How process death is confirmed on Windows,
cheaply enough to poll, is still unresolved. See
[open questions](#open-questions).

## Observation channels

Hooks are the primary channel, and the only one that can hold a tool call open
while a human decides. They have one structural weakness: they report only what
happens after they are installed, so a daemon that starts while sessions are
already running knows nothing about them until each one next emits an event.

A second channel closes part of that gap. Claude Code ships a session
enumeration, `claude agents --json`. `observed` on this machine on 2026-07-30
and re-run on 2026-08-02, both against version 2.1.220: it needs no TTY, and
it returned the live sessions with `pid`, `cwd`, `kind`, `startedAt`,
`sessionId`, and `name`. The 2026-07-30 note also listed a `status` key; no
row carried one on the re-run, so nothing may depend on it
([ADR-024](DECISIONS.md#adr-024)). It is a poll rather than a push, and it
says nothing about a pending permission, so it supplements hooks and does not
replace them.

Enumeration is no longer only a cold-start step. [ADR-028](DECISIONS.md#adr-028)
(2026-09-13) has the daemon rerun it on its own 15-second timer for as long
as it runs, outside the registry lock, so a slow or hanging enumeration call
cannot stall hook ingestion. Each run does the same three things, whether it
is the first one or the thousandth:

1. Enumerate the live sessions. A session not already bound is bound now, at
   the end of the list, by any enumeration hit or hook event, whichever
   happens first; an already-bound session is only relabelled, never
   restated.
2. Map a `busy` status to `THINKING`, where a status is reported at all.
3. Map everything else, including a status the daemon does not recognise and a
   status that is absent, to `UNKNOWN`.

On 2.1.220 no row carries a status, so step 2 never fires and every freshly
bound session lands in step 3. The rule is kept rather than deleted because
it costs nothing if the key comes back, not because state recovery works
today. What this channel actually buys is binding and labelling, not state:
a session enumeration alone finds is bound and named correctly, but stays
grey until a hook for it actually arrives. That is a smaller claim than the
one [ADR-017](DECISIONS.md#adr-017) made, and it is the one the observation
supports.

The daemon tracks that wait as `heard` on the session: false at binding,
whether by enumeration or by restoring from disk, and set true on the
first hook event received in this run. The state value stays `UNKNOWN`
either way, so this is bookkeeping for the surface's word, not a new state;
the row itself reads "not heard yet" instead of "unknown" for exactly the
session this paragraph describes, per
[ADR-029](DECISIONS.md#adr-029) and [UI_SPEC.md](UI_SPEC.md#row-anatomy).

A bound session leaves the list when it ends, or when a *successful*
enumeration run no longer lists it and it has had no hook event for 60
seconds. A failed enumeration call (a non-zero exit that is not the known
255-on-success case, or output that does not parse) prunes nothing: missing
information is never grounds for removing a row. This replaces the earlier
six fixed, manually filled slots with an unbounded list that a session can
join or leave entirely on its own ([ADR-028](DECISIONS.md#adr-028)).

Step 3 is not a formality. `IDLE` is the one guess that looks like knowledge:
a white tile says "nothing here needs you", which is exactly the claim the
daemon cannot make about a session it has never observed. The daemon never
guesses idle.

This narrows [ADR-005](DECISIONS.md#adr-005), which named hooks as the status
source. ADR-005 stands as written; [ADR-017](DECISIONS.md#adr-017) supersedes
that part of it.

One limitation belongs here and not only in the adapter. `documented`: the
switches that turn hooks off (`disableAllHooks`, `--safe-mode`, `--bare`) turn
the status line off with them, so both push channels die together and
silently. The enumeration survives all three but has an off switch of its own.
When the daemon can enumerate a session and has never received a hook from it,
the tile must say that hooks are disabled rather than sit grey with no
explanation. See
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md#known-limitations).

## Modes and hosts

The mode is a property of a session, not of the application. A single surface
can show attached sessions and hosted sessions side by side, and the tile should
make clear which is which.

Mode is not the whole story. It says who started a session; the **host** says
what is holding its process, and that is what decides which controls can act
on it. The two are separate axes, and attached mode spans two hosts: a `pty`,
whether in its own terminal window or an editor's, and `vscode-extension`,
where the process runs under the editor with no window of its own. Capabilities
therefore belong to a session rather than to an adapter. See
[DECISIONS.md](DECISIONS.md#adr-023) and
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types).

### Attached mode

Deckhand watches a `claude` session that you started, in a terminal or in the
editor.

- Status observation: **full**, through hooks, with the enumeration above as a
  second channel. The host makes no difference: hooks come out of `claude.exe`
  and not out of whatever holds its pipes.
- Approve and deny: **full**, through the `PreToolUse` hook, subject to the
  session's permission mode. Observed working on a `vscode-extension` host
  against 2.1.220.
- Send and continue: **unproven on a `pty` host, absent on a
  `vscode-extension` host.** The documented channels deliver at a turn
  boundary, never into an idle session, which is exactly when a person wants
  to type, and none has been observed. Inside the extension there is no such
  channel at all and stdin belongs to the editor. `send_prompt` is `false` on
  both. See [CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md),
  [DECISIONS.md](DECISIONS.md#adr-020), and
  [DECISIONS.md](DECISIONS.md#adr-023).
- Interrupt: no channel proven from outside the session. On a `pty` host an
  opt-in fallback synthesises keystrokes at the session's terminal window; it
  is off by default and clearly marked as unreliable. On a `vscode-extension`
  host there is no window to type into, so the fallback does not exist.

### Hosted mode

Deckhand starts and owns the session through the Claude Agent SDK.

- Everything works, including sending prompts.
- The cost is that the session has no terminal UI of its own. Something on
  the surface would have to become the only place to read the transcript,
  and the list-plus-raise design [ADR-028](DECISIONS.md#adr-028) settled on
  does not have that place yet; it is a significant amount of surface
  Deckhand would have to design and build well, and needs its own ADR when
  Phase 4 gets there.

Attached mode is built first because it is the one that improves a workflow that
already exists. Hosted mode is Phase 4.

## The adapter boundary

The target boundary keeps runtime details out of the daemon. Adapters
implement the contract in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md). The
Claude Code adapter is the reference implementation and is documented in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

The current Phase 1 code still parses Claude hook payloads inside the
registry and state machine. The proposed
[OpenAI integration plan](OPENAI_INTEGRATION_PLAN.md) stages that
extraction before adding a second runtime. Its requirements are a plan,
not evidence that the Rust adapter interface exists today.

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
 4. Tile goes amber with kind `permission`; Approve and Deny become enabled
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

What an `ask` actually reaches depends on the session's permission mode, which
is a property of the session and not something Deckhand sets. `documented`: in
`manual` it returns the decision to a human; in `auto` it returns it to
Claude Code's own classifier, which is a second gate Deckhand neither controls
nor sees; in `dontAsk` it becomes a denial. Handing back is still the safe
direction in all of them, and the fail-closed answer is still `ask` and never
`allow` ([DECISIONS.md](DECISIONS.md#adr-006)). The mode travels on
`SessionInfo`, the tile shows it as text rather than colour, the six values
are listed in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types), and what each
of the six does to a Deckhand `ask`, including the three where the answer is
"not observed, and not asserted", is in
[SECURITY_MODEL.md](SECURITY_MODEL.md#1-fail-closed-in-the-correct-direction).

## The surface

The webview draws and takes pointer input. It holds no authority and no
inference. It receives state and sends intents.

Two window properties are hard requirements rather than preferences:

1. **Always on top.**
2. **Never takes focus.** Deckhand's own window is never activated: a click on
   it must not hand focus to the board. Since ADR-027 a tile click raises the
   clicked session's host window, which is the one focus change the surface
   makes, and it goes to the session, never to Deckhand. On Windows the Tauri
   and Qt-level flags are not sufficient on
   their own: the sibling project `alpha-osk` had to apply
   `WS_EX_NOACTIVATE | WS_EX_TOPMOST` through a raw `SetWindowLongW` call, and
   reapply it whenever the window becomes visible. Deckhand has to reproduce
   that in Tauri.

This was prototyped before anything else was built, exactly because a
failure would have made the stack choice wrong while it was still cheap to
change. The spike passed on Windows 11 and the mechanism it proved, one
extended-style pass at setup, is what the Phase 1 window ships. See
[DECISIONS.md](DECISIONS.md#adr-025) and
[DECISIONS.md](DECISIONS.md#adr-002).

The window itself is a vertical list, about 360 logical pixels wide, with
its height following the row count at 64 px per row plus a 52 px header
([ADR-031](DECISIONS.md#adr-031); the header was 64 px under
[ADR-029](DECISIONS.md#adr-029), and 48 px and 56 px before that),
clamped into the monitor's work area so it can never render partly
off-screen. Since [ADR-030](DECISIONS.md#adr-030) the row count that
sizing uses is `visible_row_count`, the visible rows once the Hide grey
filter is applied, not the bound count; the `T_unknown` watchdog (see
[Liveness, by open operation](#liveness-by-open-operation)) that can move
a session into `unknown` while the filter is on routes through the same
resize path, so a session going quiet under a hidden filter shrinks the
window exactly as toggling the control would. A saved
position is checked against the monitors actually connected at startup
before it is trusted: a position saved on a monitor that is no longer
attached is discarded in favour of a position inside the current work
area, rather than placing the window off every visible screen. The window
match the raise uses, see
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#agent-keys-to-session-rows),
excludes Deckhand's own window from its candidates, so a title match can
never find the board itself. Both fix findings from the same window and
raise review; recorded together in [ADR-028](DECISIONS.md#adr-028).

Reveal, the raise this window match performs, does not apply one scored
match to every host. It classifies the session's pid first, by walking
its parent chain (a Toolhelp32 snapshot, at most eight hops) for the
first ancestor that is a host Reveal knows: `Code.exe` for VS Code,
`WindowsTerminal.exe` for Windows Terminal, anything else falling back to
a plain console. That split exists because a console session's process
owns its window one to one, while a Windows Terminal or VS Code session
shares one owning process across every window on the machine, so the
same pid-and-title score that finds a console exactly can tie or
misidentify a window on either of the other two. A console is matched by
briefly attaching to it (`AttachConsole`); a Windows Terminal session is
raised only when exactly one Terminal window is open; a VS Code session
is matched against the workspace folders named in
`~/.claude/ide/*.lock`, and, on a match, VS Code's own CLI is run against
that folder ahead of the window raise. Across every host, a tie at the
top score is now a miss rather than a guess. See
[CONTROL_MAPPING.md](CONTROL_MAPPING.md#agent-keys-to-session-rows) for
what this means as a control and [DECISIONS.md](DECISIONS.md#adr-032)
for the full record, including the unverified console path and the two
tab-targeting gaps, inside Windows Terminal and inside a VS Code window,
that stay out of reach from outside either editor.

## Stack

Tauri v2, Rust daemon, TypeScript frontend. Recorded with its alternatives and
its risks in [DECISIONS.md](DECISIONS.md#adr-002).

## Persistence

| Data | Where | Notes |
| --- | --- | --- |
| Settings | Local config directory, `settings.json` | Portable, hand-editable. Holds `hide_unknown`, the Hide grey toggle's state ([ADR-030](DECISIONS.md#adr-030)); a missing field or a corrupt file loads as `false` |
| Session bindings | Same | An ordered list, by session id, which survives restarts. A legacy six-slot `bindings.json` loads by dropping its null slots and keeping the rest in order ([ADR-028](DECISIONS.md#adr-028)) |
| Approval audit log | Local, append-only, optional | Off by default. If Deckhand approves tool calls, being able to answer "what did I approve" is worth having |
| Session transcripts | Not stored | Deckhand reads them where they already are and copies nothing |

Nothing leaves the machine. There is no telemetry, no account, and no network
egress other than loopback.

## Open questions

These are real and unresolved. They are tracked in [TODO.md](../TODO.md).

1. **Hook overhead at several concurrent sessions.** A subprocess per tool
   call across many busy sessions could be noticeable, more so now that the
   list is unbounded rather than capped at six. Needs measuring before the
   design is trusted. If it is too slow, the fallback is to hook only the events
   needed for status and gate permissions on a narrower matcher.
2. **Whether `ERROR` is detectable at all.** Amber and blue and green are
   straightforward. A failed turn may not surface as a distinct hook event. If
   it does not, red may only ever mean "the process died", and the spec should
   say so honestly rather than promise a colour that never lights. A candidate
   event is now `documented` (`StopFailure`), but nothing has been observed
   firing here, so this question stays open and red stays a narrow promise.
3. **Confirming process death on Windows** cheaply enough to poll.
4. **Whether the terminal keystroke fallback is worth shipping at all.** It may
   be that attached mode should simply not offer send, and that wanting to send
   is the reason to use hosted mode.
5. **Whether one daemon should serve several surfaces**, for example a second
   window on a tablet.
6. **Binding stability across `--resume`.** Resuming appears to continue under
   the same session id, but a tile pointing at a session that forked needs
   defined behaviour.
