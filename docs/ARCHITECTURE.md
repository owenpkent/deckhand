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
| Watchdog | Same binary, `deckhand.exe --watchdog <pid>` | Runs as long as the daemon does | Waits on the daemon's exit code and relaunches it after a crash; see [Process lifecycle](#process-lifecycle) |

The daemon and the surface ship in one Tauri application. They are described
separately because the daemon must keep working while the window is hidden, and
because a future headless or remote surface should be able to attach to the same
daemon. The watchdog is a fourth, windowless process spawned by every launch
([ADR-037](DECISIONS.md#adr-037)); two processes named `deckhand.exe` run
while the board is up.

### Why a separate hook shim

Hooks fire as short-lived subprocesses, potentially on every tool call across
six sessions. Whatever they run has to start fast. The shim does one thing:
forward stdin, return stdout. It holds no state and knows nothing about
Claude Code semantics. All interpretation happens in the daemon.

Process spawn cost is the main performance risk in this design. See
[open questions](#open-questions).

### Process lifecycle

What a launch of `deckhand.exe` does, in order, before anything else in this
document applies ([ADR-037](DECISIONS.md#adr-037)):

1. **Claim the instance mutex.** The process creates the named kernel mutex
   `Local\Deckhand.Instance` and holds it for its lifetime; the kernel drops
   the handle on any exit, crash included, so the name is always free again
   for the next launch. If the mutex already exists, this launch is a second
   copy: it raises the first copy's window to the top without activating it
   (the no-focus-steal surface, [ADR-025](DECISIONS.md#adr-025)) and exits
   with code 0. Nothing else is shared between the two copies.
2. **Spawn the watchdog.** Once the mutex is held, the app spawns
   `deckhand.exe --watchdog <own pid>` as a detached, windowless child. That
   mode opens a handle on the parent, waits for it to exit, and reads its
   exit code: 0 is a clean exit (Quit, or the OS ending the session) and the
   watchdog simply ends; anything else is a crash, and the watchdog
   relaunches `deckhand.exe` and ends, the new copy spawning its own
   watchdog in turn. A watchdog that cannot open its parent or cannot spawn
   does nothing; the app runs unguarded rather than not at all. This
   watchdog is unrelated to the `T_unknown` watchdog named later in this
   document, which times out a session's colour, not the app's own process.
3. **Start the daemon's own subsystems**: the loopback HTTP ingest endpoint
   ([Transport](#transport)), the two-second liveness tick, and the
   fifteen-second scan ([Observation channels](#observation-channels)).

Restarts are rate-limited by an append-only ledger,
`%LOCALAPPDATA%\deckhand\watchdog.log`, one line per decision: three
restarts within ten minutes is a crash loop, and the watchdog writes
"gave-up" and stops rather than flicker the board forever. The watchdog
holds a handle on the app, not the reverse, so the app never waits on it and
a dead watchdog costs nothing but the restart guarantee.

Start with Windows ([ADR-033](DECISIONS.md#adr-033)) is unchanged: the Run
key still launches `deckhand.exe` with no arguments, and the watchdog is a
consequence of any launch, not a second registration.

The watchdog is spawned with breakaway from any job object it inherits, so a
copy launched from a terminal survives that terminal closing, where the job
allows breakaway. Where the job forbids it, the watchdog is spawned inside
the job and dies with the terminal, which is the pre-ADR-037 behaviour, not
a regression.

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
| `ERROR` | Red | The turn failed | You select the tile, or the session recovers |
| `ENDED` | Off | A clean `SessionEnd`, or the process exiting without one, confirmed by a held process handle ([ADR-035](DECISIONS.md#adr-035)) | Rebound |
| `UNKNOWN` | Grey | The daemon cannot currently tell | Any authoritative event arrives |

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

`ENDED` absorbs stragglers on the same principle. Once a session reaches
`ENDED`, only a session-start event (a resume) is taken as a state change;
every other event is ignored outright, not merely processed to no visible
effect. A `Stop` or a tool-failure event delivered late, or racing the
session-end event itself, must not flip a session that has already ended
back to `COMPLETE`, `THINKING`, or `ERROR`, and must not re-list it: list
membership follows the state left after an event is applied, so a straggler
that got through this rule would put a dead session back on the board.

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

Liveness now has a second, independent channel: a held process handle
([ADR-035](DECISIONS.md#adr-035)). When a scan reports a pid for a session,
the daemon opens it with `OpenProcess(SYNCHRONIZE)` and keeps the handle for
the session's life, opened only from a scan sighting and never from a pid
restored from disk, since a restored pid may already name another process.
The two-second tick polls the handle with a zero-timeout wait. A held handle
also blocks Windows from reusing the pid, so no start-time check is needed.

One deadline, not two, and now narrower for a session the daemon holds a
handle for:

- **`T_unknown`, default 900 s**, measured from the last event of any kind.
  On expiry the session moves to `UNKNOWN`, never to `ERROR`, because a long
  tool call is normal and a wrong red costs more than an honest grey.
- The stale clock, meaning "nothing has been heard for a while", is suspended
  while an operation is open. `T_unknown` is not suspended by anything.
- For a session with a live handle, silence stops meaning degradation:
  `IDLE`, `COMPLETE`, `ERROR`, and `NEEDS_INPUT` hold for as long as the
  process lives. The exception is `THINKING`: a turn in flight produces hook
  events, so a `THINKING` session with no hook for `T_unknown`, whose latest
  scan status is not `busy`, `shell`, or `waiting`, still moves to `UNKNOWN`,
  because the two channels disagree and the colour cannot be trusted. Most
  of the time this path is moot: the [ADR-036](DECISIONS.md#adr-036)
  tie-break above resolves a `THINKING`-versus-`IDLE` disagreement in about
  thirty seconds whenever the scan carries a status at all, so the full
  fifteen-minute wait is left for the case where the scan reports no status
  for the session. A session with no handle (no pid known, or the handle
  could not be opened) keeps the old, unnarrowed rule, except that a
  successful scan sighting now also counts as an event of any kind, so
  `T_unknown` runs from the later of the last hook and the last sighting.

The asymmetry is the whole point. A terminal killed mid-tool-call leaves an
operation open that nothing will ever close, so letting an open operation
suspend `T_unknown` as well would pin that tile blue until the daemon
restarted. There is no second stale tier and no stale badge: one deadline, one
grey. Recorded in [DECISIONS.md](DECISIONS.md#adr-016), narrowed by
[ADR-035](DECISIONS.md#adr-035).

A clean exit moves the session to `ENDED`. Process death confirmed by the
held handle, without a `SessionEnd`, moves it to `ENDED` too, exactly as
`SessionEnd` would, and not to `ERROR`: a red tile with no exit path would
sit on the list until the daemon restarted, and a crash is already visible
in the host window the row points at. This resolves the open question below
on confirming process death cheaply enough to poll: a held handle and a
zero-timeout wait cost microseconds per session per tick.

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
row carried one on the 2026-08-02 re-run, so [ADR-024](DECISIONS.md#adr-024)
narrowed what depended on it. Every row carries `status` on the installed
2.1.270: `busy` and `idle` were observed live on 2026-09-15, and `shell` and
`waiting` sit beside them in the CLI's own validator list, unobserved
([ADR-035](DECISIONS.md#adr-035)). The registry the command reads,
`%USERPROFILE%\.claude\sessions\<pid>.json`, also carries `procStart`, a
process identity the CLI checks before listing a row, so a session the scan
lists has a live process at the moment of listing. It is a poll rather than a
push, and it says nothing about a pending permission, so it supplements hooks
and does not replace them.

Enumeration is no longer only a cold-start step. [ADR-028](DECISIONS.md#adr-028)
(2026-09-13) has the daemon rerun it on its own 15-second timer for as long
as it runs, outside the registry lock, so a slow or hanging enumeration call
cannot stall hook ingestion. Each run does the same things, whether it is the
first one or the thousandth:

1. Enumerate the live sessions. A session not already bound is bound now, at
   the end of the list, by any enumeration hit or hook event, whichever
   happens first; an already-bound session is only relabelled, never
   restated. A pid the daemon holds no handle for yet gets one opened now,
   per the liveness rule above ([ADR-035](DECISIONS.md#adr-035)). Relabelling
   follows one rule regardless of which channel saw the session first
   ([ADR-038](DECISIONS.md#adr-038)): a label is the scan's own `name` when
   the scan provides one, and the directory name otherwise. Whether the
   current label came from a directory is tracked as its own flag,
   `label_is_derived`, rather than guessed by comparing strings, since a
   session's real name could coincidentally match its directory. A derived
   label stays open to a later `name`; a real name, once seen, is never
   overwritten again.
2. Map the reported `status`, but only for a session hooks have not
   coloured: one that is `UNKNOWN`, or has never been heard from by a hook
   this run. `busy` and `shell` map to `THINKING`, `waiting` to
   `NEEDS_INPUT`, `idle` to `IDLE`; any other value, including one absent or
   unrecognised, leaves the state alone. Once a hook has coloured a session
   this step does not recolour it on a single contradicting scan: hooks
   carry what the scan cannot (green means finished and unread, amber
   carries the question, red carries the error), and a coarse `idle` must
   not erase them ([ADR-035](DECISIONS.md#adr-035)). The one exception is
   the tie-break [ADR-036](DECISIONS.md#adr-036) adds: after two
   consecutive scans contradict the hook-set colour with no hook event
   between them, about thirty seconds at the fifteen-second rescan, a
   `THINKING` session the scan reports `idle` moves to `IDLE`, clearing its
   open operations and child ledger, and an `IDLE`, `COMPLETE`, or `ERROR`
   session the scan reports `busy` or `shell` moves to `THINKING`. Any hook
   event resets the count. This step still never produces `COMPLETE`, still
   never touches `NEEDS_INPUT`, and `waiting` never triggers the tie-break
   in either direction.

On 2.1.220 no row carried a status, so step 2 never fired and every freshly
bound session stayed `UNKNOWN` until a hook arrived. On the installed 2.1.270,
step 2 runs its full mapping, so a session the scan finds idle, busy,
waiting, or in a shell is coloured correctly on the next scan instead of
sitting grey. What this channel buys is no longer only binding and
labelling: within that mapping, and only until a hook has something more
specific to say, it buys state too. This is a larger claim than
[ADR-017](DECISIONS.md#adr-017) made and [ADR-024](DECISIONS.md#adr-024) had
to take back; [ADR-035](DECISIONS.md#adr-035) re-earns it, on the strength of
`status` actually being observed this time.

The daemon tracks whether a hook has spoken for a session this run as
`heard`: false at binding, whether by enumeration or by restoring from disk,
and set true on the first hook event received in this run. `heard` gates
step 2's colouring, not only the surface's word choice: the row reads "not
heard yet" instead of "unknown" for exactly the session this paragraph
describes, per [ADR-029](DECISIONS.md#adr-029) and
[UI_SPEC.md](UI_SPEC.md#row-anatomy).

A bound session leaves the list on one of three events
([ADR-035](DECISIONS.md#adr-035)): a `SessionEnd`, the daemon's held handle
reporting the process has exited, or, only for a session the daemon holds no
handle for (no pid known, or the handle could not be opened), a *successful*
enumeration run no longer listing it after 60 seconds with no hook event. A
failed enumeration call (a non-zero exit that is not the known 255-on-success
case, or output that does not parse) prunes nothing: missing information is
never grounds for removing a row. This replaces the earlier six fixed,
manually filled slots with an unbounded list that a session can join or
leave entirely on its own ([ADR-028](DECISIONS.md#adr-028)).

A bound session can also be left out of the list without being unbound at
all: supersession ([ADR-038](DECISIONS.md#adr-038)). The VS Code extension
was observed keeping an older session's `claude.exe` alive, for hours, after
a newer session started in the same window, so enumeration lists both and
the board would otherwise show what looks like a duplicate row. Session O is
hidden while some other bound session N satisfies all of: both have a known
pid; both are hosted by the VS Code extension
([ADR-023](DECISIONS.md#adr-023)); both processes share the same direct
parent pid, resolved once when the pid is learned; both share a working
directory, compared case-insensitively with separators normalised; N
started after O, by the scan's `startedAt` when every session sharing that
window and directory reports one and by first-seen time otherwise, so one
clock orders the whole group; O is `IDLE`, `COMPLETE`, or `UNKNOWN`; and N
is not `ENDED`. A session that is `THINKING`, `NEEDS_INPUT`, or `ERROR` is
never hidden this way, whatever is newer, and a console or Windows Terminal
session is never hidden either, since the observed case is specific to the
extension host. `app/src-tauri/src/supersede.rs` holds the rule as a pure
function, recomputed by `Registry::snapshot` and by the window-sizing path
on every call rather than cached: nothing is remembered, so O reappears, in
its old position and selection, the moment it stops qualifying. A hidden row
counts toward nothing: not the header counts, not the Hide grey count, not
the window height.

`IDLE` from the scan is a read, not a guess, which is worth stating plainly
since the rule used to be stricter. A white tile says "nothing here needs
you"; before 2.1.270 confirming that claim needed a hook, because no
enumeration field said so and the daemon would not invent one. The
registry's `procStart` check means a `status: "idle"` row is a live process
Claude Code itself calls idle at the moment of listing, not a guess dressed
up as one, so step 2 may colour `IDLE` for a session hooks have not spoken
for. What the daemon still never does is show `IDLE` the scan did not
report.

This narrows [ADR-005](DECISIONS.md#adr-005), which named hooks as the status
source. ADR-005 stands as written; [ADR-017](DECISIONS.md#adr-017) and
[ADR-035](DECISIONS.md#adr-035) supersede that part of it.

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
sizing uses is `visible_row_count`, the visible rows once the grey toggle
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

## Settings panel

Opened by the header's gear, replacing the session list in place
([ADR-033](DECISIONS.md#adr-033)); not a second window. `main.rs` holds
a `PANEL_OPEN` flag (not persisted, always starts closed) and a fixed
`PANEL_ROW_COUNT`; the window resizes to whichever count currently
applies through the same `resize_for_rows`/`queue_resize` path every
session-count change already used, and a hook event arriving while the
panel is open resizes to the panel's own row count rather than the
list's, so the panel is never resized out from under the owner mid-use.

New commands, every one taking no argument from the webview:

| Command | Does |
| --- | --- |
| `toggle_settings_panel` | Opens or closes the panel; returns the new open state. |
| `get_settings_snapshot` | Reads always-on-top, start-with-Windows, hooks status, and whether an installer checkout was found. |
| `toggle_always_on_top` | Flips and persists always-on-top, applies it to the real window, and reapplies the `WS_EX_NOACTIVATE` style ([ADR-025](DECISIONS.md#adr-025)) since toggling topmost can reset it. |
| `toggle_start_with_windows` | Reads the HKCU Run key fresh and either writes this exe's path over it or deletes it, never trusting a client-cached guess of the current state. |
| `reset_window_position` | Moves the window to `window::default_rect`'s placement and persists the result. |
| `repair_hooks` | `async`; reruns `scripts/install-hooks.ps1` off the webview/event thread via `spawn_blocking`, the same pattern `activate_session` uses for its reveal wait, bounded at 20 s. |

`toggle_hide_unknown` is not one of these new commands and is
unchanged: it is still called from the header's own Hide grey switch
([ADR-030](DECISIONS.md#adr-030)), which never moved into the panel,
only restyled in place ([ADR-034](DECISIONS.md#adr-034)).

## Stack

Tauri v2, Rust daemon, TypeScript frontend. Recorded with its alternatives and
its risks in [DECISIONS.md](DECISIONS.md#adr-002).

## Persistence

| Data | Where | Notes |
| --- | --- | --- |
| Settings | Local config directory, `settings.json` | Portable, hand-editable. Holds `hide_unknown` (the header's own Hide grey switch, [ADR-030](DECISIONS.md#adr-030), left there rather than moved by [ADR-033](DECISIONS.md#adr-033)) and `always_on_top` (the settings panel's Always on top row, [ADR-033](DECISIONS.md#adr-033), default `true`); a missing field or a corrupt file loads each at its own default rather than failing |
| Start with Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Deckhand` | Not mirrored into `settings.json`; the registry value itself is the only source of truth, read fresh on every panel open and every toggle ([ADR-033](DECISIONS.md#adr-033)) |
| Session bindings | Local config directory, `bindings.json` | An ordered list, by session id, which survives restarts. A legacy six-slot `bindings.json` loads by dropping its null slots and keeping the rest in order ([ADR-028](DECISIONS.md#adr-028)). Each entry also carries `derived`, whether its label came from a directory name rather than the scan's own `name`; an entry written before the flag existed loads as derived, so it takes the scan's name on the next scan ([ADR-038](DECISIONS.md#adr-038)) |
| Watchdog ledger | Local config directory, `watchdog.log` | Append-only, one line per restart decision, capped at three restarts in ten minutes before the watchdog gives up. Never leaves the machine ([ADR-037](DECISIONS.md#adr-037)) |
| Approval audit log | Local, append-only, optional | Off by default. If Deckhand approves tool calls, being able to answer "what did I approve" is worth having |
| Session transcripts | Not stored | Deckhand does not read them ([ADR-036](DECISIONS.md#adr-036)) |

Nothing leaves the machine. There is no telemetry, no account, and no network
egress other than loopback. The settings panel's Repair action and its
Start with Windows row are the two exceptions to "nothing outside
Deckhand's own data directory": Repair runs `scripts/install-hooks.ps1`,
which writes `~/.claude/settings.json`, and Start with Windows writes
the HKCU Run key above. Both stay local to the machine and both fire
only on an explicit click; see
[SECURITY_MODEL.md](SECURITY_MODEL.md) for the trust implications.

## Open questions

These are real and unresolved. They are tracked in [TODO.md](../TODO.md).

1. **Hook overhead at several concurrent sessions.** A subprocess per tool
   call across many busy sessions could be noticeable, more so now that the
   list is unbounded rather than capped at six. Needs measuring before the
   design is trusted. If it is too slow, the fallback is to hook only the events
   needed for status and gate permissions on a narrower matcher.
2. **Whether `ERROR` is detectable at all.** Amber and blue and green are
   straightforward. A failed turn may not surface as a distinct hook event,
   and process death no longer stands in for it: confirmed death now moves a
   session straight to `ENDED`, not `ERROR` ([ADR-035](DECISIONS.md#adr-035)).
   If `StopFailure` never fires, red may end up a promise nothing ever
   redeems, and the spec should say so honestly rather than claim a colour
   that never lights. A candidate event is `documented` (`StopFailure`), but
   nothing has been observed firing here, so this question stays open.
3. **Whether the terminal keystroke fallback is worth shipping at all.** It may
   be that attached mode should simply not offer send, and that wanting to send
   is the reason to use hosted mode.
4. **Whether one daemon should serve several surfaces**, for example a second
   window on a tablet.
5. **Binding stability across `--resume`.** Resuming appears to continue under
   the same session id, but a tile pointing at a session that forked needs
   defined behaviour.
