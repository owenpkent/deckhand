# Deckhand: A Mouse-Only Status Surface for Parallel Claude Code Sessions

Status: **accepted**. This is a derived technical paper, not an
authoritative source. Where it disagrees with the documents it cites, the
cited document wins (see [WORKFLOW.md](WORKFLOW.md) section 1).

**Document revision:** 1.0 (September 2026), describing Deckhand at Phase 1

**Author:** Owen Kent

**Audience:** software engineers building tooling around coding agents,
and accessibility and assistive-technology practitioners

---

## Abstract

Running several Claude Code sessions at once turns the developer into a
poller: every session that finishes a turn, asks a question, or fails has to
be noticed, and the only way to notice is to go and look. For a user whose
pointer is cheap but whose keyboard, window switching, and focus changes are
physically expensive, that polling cost compounds across a working day.
Deckhand is a small, always-on-top Windows surface, operated with a pointer
alone, that lists every live Claude Code session on the machine with a
colour, a glyph, and a state word, and brings a session's window forward in
the same click that selects it. It is a software reinterpretation of the
Codex Micro, a limited-run macropad whose status keys made agent sessions
readable at a glance.

Deckhand is built as one Tauri v2 application (a Rust daemon and a
TypeScript surface) plus a dependency-free hook shim. It observes sessions
through three channels: Claude Code's hook events, the periodic output of
`claude agents --json`, and a handle held on each session's process. It
never reads transcripts and, in its current phase, has no write authority
over any session. This paper describes the problem, the surface, the
architecture, how the three observation channels are reconciled into one
honest state per session, the Windows integration, the security model
(including the approval path that is designed but deliberately not built),
the accessibility rules that govern every interaction, the evidence behind
each integration claim, and the system's limitations. The largest of those
is that it has one user, and the reliability of hook-inferred status at a
glance, the premise of the whole project, is still being established.

---

## 1. Introduction

### 1.1 The monitoring problem

A Claude Code session spends most of its life in one of a few conditions:
working, waiting for a person, finished with output nobody has read, or
failed. A single session in a single terminal makes those conditions easy
to see. Six sessions across three repositories do not. Each one lives in
its own terminal or editor tab, and each change of condition is visible
only in that tab. The developer's choices are to switch between them on a
schedule, or to miss things.

For most people this is an annoyance measured in seconds. It is not evenly
distributed. Deckhand's author is a wheelchair user with muscular
dystrophy, for whom a pointer movement is cheap and a keypress, a window
switch, or a hunt for the right tab is not. "Just check the other
terminal" costs a few seconds every time, many times an hour. A monitoring
problem that most people tolerate is a larger problem for people who cannot
tolerate it cheaply, and that asymmetry is the reason the project exists.

### 1.2 Prior art: the Codex Micro

The Codex Micro, a limited-run macropad built by Work Louder in
collaboration with OpenAI, is the direct inspiration and is credited as
such ([ADR-001](DECISIONS.md#adr-001)). It paired agent keys bound to chat
sessions with status LEDs, command keys for actions such as approve and
deny, a dial, a stick, and push-to-talk. Its central insight, which
Deckhand keeps, is that managing several agent conversations is a
status-board problem as much as a chat problem, and that a small, fixed
surface read at a glance can beat a window manager for it.

A hardware device cannot be free, cannot outlive its production run,
does not drive Claude Code, and assumes working hands. Deckhand borrows
the interaction model and points it at a different backend and a
different constraint. It is not affiliated with Work Louder, OpenAI, or
Anthropic. Six of its seven status colours come from the device and are
frozen by [ADR-008](DECISIONS.md#adr-008).

### 1.3 Design goals

The goals below are distilled from the decision record. The ordering is
this paper's reading of how conflicts have actually been resolved, not a
ranking the project has formally adopted.

1. **Never break Claude Code.** Deckhand sits in the path of every hook
   Claude Code fires. If Deckhand is down, slow, or wrong, Claude Code must
   behave exactly as if Deckhand were not installed.
2. **Never guess.** A colour on the board is a claim. When observation
   degrades, the row turns grey (`unknown`) rather than showing a plausible
   state that might be false.
3. **Pointer only, single clicks.** No required keyboard, holds, drags,
   double-clicks, or hovers, and a 44 px minimum target
   ([ACCESSIBILITY.md](ACCESSIBILITY.md)). One exception exists and is
   recorded as a gap (section 7.3).
4. **Never steal focus.** Clicking the surface must not take keyboard
   focus away from whatever the user was typing into.
5. **Earn authority before holding it.** Observation ships first. Approving
   or denying a tool call, the capability with real consequences, waits
   until observation has proven trustworthy, and when it arrives it must
   fail towards asking a human, never towards allowing
   ([ADR-006](DECISIONS.md#adr-006)).
6. **Local only.** Nothing leaves the machine: no telemetry, no crash
   reporting, no network listener beyond loopback.

### 1.4 Non-goals

- Replacing the terminal or the editor. Deckhand points at sessions; it
  does not render their conversations.
- Sending prompts into running sessions. No channel that delivers a prompt
  into an idle attached session has been observed working, so Deckhand
  declares the capability false ([ADR-020](DECISIONS.md#adr-020)).
- Speech input ([ADR-010](DECISIONS.md#adr-010)).
- Reading session transcripts, for any purpose (section 4.7).

### 1.5 Status at the time of writing

Deckhand is in Phase 1, observation only, which began on 2026-08-02 after
a Phase 0 of specification and two feasibility spikes. The daemon, surface,
and shim exist and have watched the author's live sessions daily. Nothing
in the system can approve, deny, or send anything. The decision record runs
to 38 entries, and this paper cites them by number.

---

## 2. The Surface

### 2.1 Rows and the colour language

The surface is a frameless window 360 logical pixels wide. Below a 52 px
header it shows one 64 px row per live session, in first-seen order, and
the window grows and shrinks with the list, capped at the monitor's work
area. The list is unbounded; an earlier design with six fixed slots was
retired because automatic binding never filled them reliably across
repositories ([ADR-028](DECISIONS.md#adr-028)).

Each row carries a 30 px glyph, the session's name, a state word beneath
it, and a 4 px accent bar in the state colour down its left edge. The name
is the one Claude Code reports for the session, or its folder name until
one is reported ([ADR-038](DECISIONS.md#adr-038)). Colour is
never the only channel: every state has a distinct glyph and label, so the
board reads correctly for a colour-blind user and in greyscale.

| State | Colour | Glyph | Meaning |
|---|---|---|---|
| `idle` | white | open circle | Bound, nothing running |
| `thinking` | blue | arc spinner | A turn or tool call is in flight |
| `complete` | green | check | Finished and unread; clears on selection |
| `needs_input` | amber | hand | Waiting on a human |
| `error` | red | cross | The turn failed |
| `ended` | off | dash | The session ended; the row leaves the list |
| `unknown` | grey | question mark | Observation degraded; never a guess |

`unknown` is the only state Deckhand added to the device's vocabulary. It
exists because the alternative, holding the last known colour when evidence
stops arriving, would let a stale blue look exactly like live work.
[ADR-029](DECISIONS.md#adr-029) later split its label so that a session
the daemon has never heard from ("not heard yet") reads differently from
one that has gone quiet.

### 2.2 Header and settings

The header is also the window's drag region
([ADR-031](DECISIONS.md#adr-031)). It holds a read-only summary of state
counts, a Hide grey switch, a gear, and Quit. Hide grey filters `unknown`
rows out of the list and shows how many it is hiding on the switch itself
("3 hidden"), so the filter can never silently hide a problem
([ADR-030](DECISIONS.md#adr-030)).

The gear replaces the session list, in place, with a settings panel of two
sections ([ADR-033](DECISIONS.md#adr-033),
[ADR-034](DECISIONS.md#adr-034)). Window holds always on top (default on),
start with Windows, and reset position. Claude Code holds a hooks status
(installed, outdated, missing, or unreadable, parsed from the user's
Claude Code settings) with a Repair action that reruns the hook installer.
Hide grey stays in the header throughout: a filter that changes what the
board shows belongs where the board is.

### 2.3 One click: select and raise

Clicking a row does two things at once
([ADR-027](DECISIONS.md#adr-027)). It selects the session, which is what
clears a green `complete` back to white, and it brings the window hosting
that session to the front. The two actions were once separate targets; they
were merged because the reason to select a session is almost always to go
to it, and a second target is a second click. Section 5.2 describes how the
daemon finds the right window, and when it declines to.

### 2.4 How the surface became this small

The Phase 0 specification, written in full on 2026-07-27, mapped the whole
Codex Micro: six agent tiles, command keys for approve, deny, and continue,
a dial, a stick, push-to-talk, layers, a detail panel, and answer targets
for an agent's multiple-choice questions
([ADR-013](DECISIONS.md#adr-013)). Building Phase 1 against real sessions
showed that most of that surface either depended on write authority that
did not exist yet or duplicated the one action that mattered.
[ADR-028](DECISIONS.md#adr-028), on 2026-09-13, cut the surface to the
session list. Controls that act on a session are not merely postponed;
each needs its own decision to return.

The same week refined what remained. ADR-029 made rows taller and
two-line, added header counts, and bundled a typeface chosen for
legibility (Atkinson Hyperlegible Next). ADR-030 added Hide grey.
ADR-031 removed Move, a click-to-place alternative to dragging, and made
the header a drag bar. ADR-033 and ADR-034 added and redrew the settings
panel. The result is closer to a status light than to a control panel,
which is what the first months of use suggested it should be.

---

## 3. System Architecture

### 3.1 Processes

```
 Claude Code session
   | hook fires, JSON on stdin
   v
 deckhand-shim.exe            (one per hook, lives milliseconds)
   | POST /hook, 127.0.0.1:<ephemeral port>, bearer token
   v
 +------------------------------+
 | deckhand.exe (daemon)        | --- every 15 s ---> claude agents --json
 | registry, state machines     | --- 2 s tick -----> session process handles
 +------------------------------+
   | Tauri IPC (snapshots down, commands up)
   v
 surface (TypeScript webview)

 deckhand.exe --watchdog <pid>  (same binary, waits on the daemon)
```

| Process | Language | Lifetime | Authority |
|---|---|---|---|
| Hook shim | Rust, standard library only | Milliseconds, one per hook fire | Forwards one event |
| Daemon | Rust | While the surface is open | Owns all state |
| Surface | TypeScript in the Tauri webview | Same as the daemon | None; renders snapshots |
| Watchdog | Same binary as the daemon | Waits on the daemon | Relaunches after a crash |

The stack was fixed before any code existed
([ADR-002](DECISIONS.md#adr-002)): Tauri v2 for a small native window
with a web-rendered surface, Rust for a daemon that has to be cheap to
leave running, and TypeScript for the surface. The daemon and surface ship
as one application; the shim is a separate binary because Claude Code
launches it once per hook.

### 3.2 The shim: a hook that cannot hurt its host

Claude Code runs the configured hook command for every registered event
and, for some events, waits for it. That makes the hook the one part of
Deckhand that can degrade Claude Code itself, so the shim is built around
goal 1. It has no dependencies, no async runtime, no TLS, and no DNS
lookup. It reads the event from standard input, reads the daemon's port
and token from `%LOCALAPPDATA%\deckhand\daemon.json`, and posts the event
with a 300 ms connect timeout and a 700 ms I/O timeout. Whatever happens,
including a missing contact file, a refused connection, or a timeout, it
exits with code 0 and writes nothing to standard output. In Phase 1 the
shim never returns a decision.

A stale contact file left by a crash is harmless for the same reason: the
connection fails fast and the shim exits silently.

### 3.3 Ingest over loopback

The daemon listens on `127.0.0.1` on an ephemeral port, never on all
interfaces, and accepts one route, `POST /hook`, carrying a per-install
bearer token ([ADR-007](DECISIONS.md#adr-007)). The ingest path was
hardened before any real session was pointed at it: the token is compared
in constant time, bodies are capped at 8 MiB, a declared length over the
cap is refused with `413` before the body is read, and chunked bodies are
refused with `411`, also unread
([SECURITY_MODEL.md](SECURITY_MODEL.md)).

### 3.4 Daemon and surface

The surface holds no state of its own beyond what it is shown. The daemon
emits a full snapshot as a Tauri event whenever the registry changes, and
the surface calls a small set of IPC commands: activate a session, quit,
toggle Hide grey, open settings, change a setting, reset position, and
repair hooks. Decisions about sessions, had any existed, would be
accepted only over this IPC channel, never over HTTP, so a local process
that can reach the loopback port still cannot act through it.

### 3.5 Persistence and data locality

Everything the daemon keeps lives under `%LOCALAPPDATA%\deckhand`:

| File | Contents |
|---|---|
| `daemon.json` | Port and token for the shim; removed on clean shutdown |
| `bindings.json` | Ordered list of session ids and labels, so row order survives a restart |
| `settings.json` | Hide grey and always on top |
| `window.json` | Window position |
| `watchdog.log` | Append-only restart ledger (section 5.3) |

Writes are atomic (write a sibling temporary file, then rename), so neither
the shim nor a restarting daemon ever reads a half-written file. Session
state itself is not persisted: after a restart every restored row starts
grey and is recoloured by evidence, rather than resuming a colour that may
no longer be true.

---

## 4. Observation

### 4.1 Three channels, and one that is refused

Deckhand has three sources of evidence about a session, with different
strengths:

- **Hook events** are immediate and specific. They say a prompt was
  submitted, a tool is about to run, a turn stopped. They are also silent
  when nothing fires, and a missed hook leaves no trace.
- **The scan**, `claude agents --json`, run at start and every 15 seconds,
  lists every session with its id, name, working directory, process id,
  and, on Claude Code 2.1.270, a `status` of `busy`, `idle`, `shell`, or
  `waiting`. It is slow and coarse but independent of hooks
  ([ADR-017](DECISIONS.md#adr-017), [ADR-024](DECISIONS.md#adr-024),
  [ADR-035](DECISIONS.md#adr-035)).
- **A process handle**, opened on each session's `claude` process the
  first time the scan reports its pid and checked every 2 seconds. It says
  nothing about what a session is doing, only whether it still exists
  ([ADR-035](DECISIONS.md#adr-035)).

The fourth candidate, reading the session transcript from disk, is refused
by design (section 4.7).

### 4.2 Hook events to states

Twelve hook events are registered. The daemon's state machine, in
`state.rs`, maps each one as follows:

| Event | Effect |
|---|---|
| `SessionStart` | `idle`, except a `compact` source, which changes nothing |
| `UserPromptSubmit` | `thinking` |
| `PreToolUse` | `thinking`, or `needs_input` when the tool is `AskUserQuestion` |
| `PostToolUse`, `PermissionDenied` | `thinking`; closes the open tool bracket |
| `PostToolUseFailure` | `thinking`, recording the error detail |
| `Notification` | `needs_input` or `idle`, by notification type |
| `Stop` | `complete`, or deferred while subagents are live |
| `StopFailure` | `error` |
| `SubagentStart`, `SubagentStop` | Maintain the live child ledger |
| `SessionEnd` | `ended`, except `clear` and `resume` reasons |

Two details matter more than the table suggests. First, liveness during a
turn is judged by open-operation bracketing (a tool call that has started
and not finished) rather than by how long a turn has run, so a long build
does not look stalled ([ADR-016](DECISIONS.md#adr-016)). Second, a
`Stop` from a session with live subagents does not turn it green until the
last child finishes; otherwise the board would announce "done" while work
continued ([ADR-019](DECISIONS.md#adr-019)).

### 4.3 Grey, and when it is allowed

A session with no process handle turns grey after `T_unknown`, fifteen
minutes without any evidence. A session with a live handle is treated
differently, because the handle already proves it exists: silence never
greys `idle`, `complete`, `error`, or `needs_input`, since waiting quietly
is exactly what those states look like. Only `thinking` can still grey, and
only when hooks have been silent past `T_unknown` and the scan does not
report the session as busy. Grey never becomes `error` on its own; a
timeout is evidence of silence, not of failure.

### 4.4 Liveness from the process

Before [ADR-035](DECISIONS.md#adr-035), a session that crashed, or whose
terminal was closed, sent no `SessionEnd` and left a red or grey row behind
until something else pruned it. The daemon now opens a handle with only the
`SYNCHRONIZE` right when the scan first reports a session's pid, and polls
it with a zero-timeout wait on the existing two-second tick. When the
process is gone, the session moves straight to `ended` and leaves the list.

Three choices keep this honest. The handle is opened only from a live scan
sighting, never from a pid restored from disk, so a recycled pid cannot be
mistaken for the session. Holding the handle prevents Windows from reusing
the pid while it is held. And a vanished process becomes `ended`, not
`error`: a crash is already visible in the window that hosted it, and a
red row that no action can clear would only add noise.

### 4.5 When hooks and the scan disagree

Hooks are trusted first, because they are immediate. But a missed hook can
leave a session blue long after its turn ended.
[ADR-036](DECISIONS.md#adr-036) lets the scan break the tie: when two
consecutive scans contradict a hook-set colour with no hook arriving
between them, about thirty seconds, the scan's reading wins. A blue session
the scan twice reports `idle` becomes white, clearing its open operations
and any deferred completion. A white, green, or red session the scan
twice reports `busy` or `shell` becomes blue.

The tiebreak is deliberately narrow. It never produces `complete`, because
"finished and unread" is a claim only a `Stop` hook can make. It never
touches `needs_input`, and a scan `waiting` never triggers it, because
hiding a request for a human is the costliest error the board could make.
Where no hook has ever coloured a session, the scan's `status` colours it
directly.

### 4.6 One live session per editor conversation

The scan lists processes, not conversations a user still considers open.
On 2026-09-15 the author's board showed what looked like duplicate rows:
in each of two VS Code windows, an older idle session and a newer busy one
shared a working directory and the same extension host process, and the
older one had gone quiet a few minutes before the newer one started. The
extension had kept the older `claude` process alive for hours, so its
handle kept its row. How the older process came to outlive its
conversation is not confirmed.

[ADR-038](DECISIONS.md#adr-038) hides such a session. An older session is
superseded when a newer one shares its VS Code extension host process and
its working directory, and the older one is idle, done, or grey. A session
that is working, waiting, or failed is never hidden, and terminal-hosted
sessions are never superseded. The rule is recomputed from the registry
every time it is needed and remembers nothing, so a hidden session returns
to its old place the moment it does anything. Its cost is stated in the
decision: two conversations genuinely open side by side in one window and
folder look identical to the observed case, and the idle older one is
hidden until it acts.

### 4.7 Why there is no transcript fallback

Claude Code writes each session's conversation to a JSONL file, and the
Phase 0 design planned to read it as a last resort when hooks went silent.
[ADR-005](DECISIONS.md#adr-005) had already ruled transcripts never
load-bearing. [ADR-036](DECISIONS.md#adr-036) retired the fallback
entirely, unbuilt. The scan's `status` answers the question the fallback
existed for ("is this session actually still working?") from a documented
command, within one fifteen-second rescan, and no missed hook has been
observed in daily use. Not reading transcripts also keeps the most
sensitive data on the machine, the conversations themselves, out of
Deckhand's reach entirely.

---

## 5. Windows Integration

### 5.1 A window that does not take focus

An always-on-top surface that steals keyboard focus when clicked would
make every glance at the board an interruption. The window spike of
2026-08-02 ([ADR-025](DECISIONS.md#adr-025)) established that Tauri's
`alwaysOnTop` and `focus: false` settings give the window
`WS_EX_TOPMOST` but not `WS_EX_NOACTIVATE`. One `SetWindowLongPtrW` call
at setup adds the missing style, and a synthetic click on the window was
then received without the foreground window changing. Toggling always on
top can reset the extended style, so the daemon reapplies it after every
toggle.

### 5.2 Reveal: finding a session's window

A session's process does not own a window of its own in the common cases,
so raising "the session" means finding whatever hosts it. The daemon walks
the session process's parents, up to eight hops, and classifies the host
([ADR-023](DECISIONS.md#adr-023), [ADR-032](DECISIONS.md#adr-032)):

- **Console.** The daemon attaches to the session's console and raises
  that console window, an exact match.
- **Windows Terminal.** If exactly one Terminal window is open, it is
  raised. With two or more, the daemon cannot tell which holds the
  session, because Windows Terminal exposes no way to target a tab from
  outside (`microsoft/terminal#19783`, closed as not planned), so it
  declines.
- **VS Code.** The daemon reads the Claude Code extension's lock files for
  each editor's process id and workspace folders, picks the editor whose
  workspace is the longest ancestor of the session's directory, asks that
  editor to focus the folder, and raises its window. It never contacts the
  extension's local server.

Across all three, a tie is a miss. When two candidates score equally, the
daemon raises neither, rather than letting enumeration order decide. A
wrong window raised with confidence costs the user more than no window
raised at all. The daemon's own window is always excluded.

### 5.3 Single instance and crash recovery

[ADR-037](DECISIONS.md#adr-037) gave the daemon a process lifecycle. A
named kernel mutex, `Local\Deckhand.Instance`, makes the application
single instance; a second launch raises the first window without
activating it and exits. The mutex handle is never closed, so the kernel
releases it on any exit, including a crash.

Every launch also spawns the same binary as `deckhand.exe --watchdog
<pid>`, detached from the launching job where Windows allows. The watchdog
waits on the daemon's process. A zero exit code is a clean quit and the
watchdog exits with it. Any other code is a crash, and the watchdog
relaunches the application, recording the decision in
`%LOCALAPPDATA%\deckhand\watchdog.log`. Three restarts within ten minutes
is treated as a crash loop: the watchdog records that it gave up and stops,
rather than relaunching a failing binary forever. The ledger is local and
never transmitted.

### 5.4 Start with Windows and hook repair

Start with Windows writes a value named `Deckhand` under the current
user's `Run` registry key, independent of the watchdog. Hook repair runs
`scripts/install-hooks.ps1`, which registers the shim for all twelve
events in the user's Claude Code settings, backing the file up on its first
write. Repair is bounded at twenty seconds and runs off the UI thread. It
currently works only when the executable runs from a source checkout,
which is the only way Deckhand is installed today.

---

## 6. Security Model

### 6.1 Phase 1 holds no authority

The simplest security property Deckhand has is the one most worth
stating: in Phase 1 nothing in it can approve, deny, send, or interrupt.
The shim writes nothing to standard output, so no hook response can carry a
decision even by accident. The daemon observes and renders. The worst a
compromised or buggy Phase 1 Deckhand can do to a session is misreport it.

### 6.2 Trust boundaries

The trusted path runs from the user's pointer, through the surface, to the
daemon, and from Claude Code, through the shim, to the daemon over
loopback. Other local processes are untrusted. Claude Code itself, the
model, Anthropic's services, and an attacker already running code as the
same user are out of scope ([SECURITY_MODEL.md](SECURITY_MODEL.md)).

One gap inside that boundary is known and accepted for Phase 1 only: any
process running as the same Windows user can read the token file and post
fabricated events. In an observation-only system that can make a row
lie; it cannot make a session do anything. It must be closed before
Phase 2.

### 6.3 Designed, not built: the approval path

The Phase 2 approval path is fully specified and intentionally unwired.
A `PreToolUse` hook is the only mechanism
([ADR-006](DECISIONS.md#adr-006)): the shim would hold the hook open, the
daemon would put the request on the surface, and the human's click would
return `allow` or `deny`. The rule that governs it is that every exit path
not ending in a human decision resolves to `ask` or to no output, never to
`allow`. That covers a request that times out, a surface that is not
running, a daemon shutting down, and a shim that cannot reach the daemon.
`ask` hands the decision back to Claude Code's own permission flow, whose
destination depends on the session's permission mode
([ADR-018](DECISIONS.md#adr-018), [ADR-022](DECISIONS.md#adr-022)).

The gating hook's output is restricted to three fields: the event name,
the decision, and a reason ([ADR-015](DECISIONS.md#adr-015)). Fields that
would rewrite a tool's input or grant standing permissions are forbidden.
The default gate is also narrow rather than a wildcard over every tool
([ADR-014](DECISIONS.md#adr-014)).

Since [ADR-028](DECISIONS.md#adr-028) removed the command keys that were
to carry approve and deny, the control shape for Phase 2 is itself an open
decision. The fail-to-`ask` rule does not depend on it.

### 6.4 Residual risks

- A same-user process can drive the surface's IPC or synthesise pointer
  input. An always-on-top window can also overlap other applications'
  dialogs.
- Repair edits the user's Claude Code settings without showing a diff
  first.
- Raising a terminal can land a click on a permission prompt already on
  screen, answering it unintentionally (`anthropics/claude-code#77827`).
  Deckhand's raise triggers exactly that situation. The risk is recorded in
  [ADR-032](DECISIONS.md#adr-032) and is unmitigated.
- How Claude Code resolves conflicting decisions from two gating hook
  entries is undocumented. That matters only once a gate exists.

---

## 7. Accessibility as a Requirement

### 7.1 The rule set

[ACCESSIBILITY.md](ACCESSIBILITY.md) is written as requirements, and it
wins every conflict with any other document. Deckhand must be fully
operable with a pointer alone, through single clicks on stationary
targets. Press-and-hold, drag, double-click, hover-only reveals, chorded
input, and keyboard input are forbidden as the only path to anything.
Targets are at least 44 px, a floor taken from WCAG's enhanced target size,
not a target to aim for: rows are 64 px and the gear and Quit buttons are
44 px squares. Tooltips never carry information that is unavailable
otherwise ([ADR-021](DECISIONS.md#adr-021)).

### 7.2 The economics of a click

The rules follow from a cost model rather than a checklist: a glance beats
a click, one decision should cost one click, pointer travel should be
short, and the interface must never manufacture a click it could have
avoided. The model also shaped priorities. In one corpus of 240 sessions
on the author's machine, an agent's multiple-choice question was roughly
fifteen times more common than a tool denial. That is a measurement of one
person's work, not a general finding, but it is why answering questions
was once designed as a first-class surface action, and why its removal in
ADR-028 was a deliberate trade rather than an oversight.

### 7.3 The drag exception

[ADR-031](DECISIONS.md#adr-031) removed Move, the click-to-place control
that repositioned the window without dragging, and made the header a drag
region. Dragging is now the only way to move the window, which breaks the
rule in section 7.1. The decision records it as an owner-approved
exception, not a compliant default, and as an open accessibility gap until
a click-based control returns with its own decision. Reset position in the
settings panel is a partial mitigation: it always returns the window to a
known place in one click.

---

## 8. Evidence and Verification

### 8.1 Evidence tiers

Every claim about Claude Code's behaviour in the specification carries a
stamp: `observed` means seen on this machine, `documented` means read in
official documentation but not seen, and `unverified` means neither. The
stamps are not upgraded without the observation that justifies them. The
reference adapter document, [CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md),
carries a partial verification stamp against Claude Code 2.1.220. The
scan's `status` key is a separate and narrower observation, against 2.1.270.

### 8.2 What has been observed

| Claim | Evidence |
|---|---|
| No-focus-steal window in Tauri on Windows 11 | Spike, 2026-08-02 ([ADR-025](DECISIONS.md#adr-025)) |
| A `PreToolUse` deny is honoured and blocks the call | Live session, 2026-08-02 |
| 10 of 12 registered hook events fire, with full payloads | Capture hook, from 2026-08-02 ([ADR-026](DECISIONS.md#adr-026)) |
| `PostToolUseFailure` carries `error` and `is_interrupt`, not the documented `error_type` | Live capture ([ADR-026](DECISIONS.md#adr-026)) |
| Scan `status` values `busy` and `idle` | Claude Code 2.1.270, 2026-09-15 ([ADR-035](DECISIONS.md#adr-035)) |
| `claude agents --json` exits 255 while printing valid output | Claude Code 2.1.220 |

Live validation corrected the documentation more than once, which is the
argument for the stamps. The payload shape of `PostToolUseFailure` was
documented wrongly, and an earlier run on 2026-08-02 saw no `status` key
on scan rows at all, a result later versions overturned. The daemon
therefore treats the scan's exit code as meaningless and parseable output
as the success signal.

### 8.3 What has not been observed

- The `Notification` and `StopFailure` hook events have never fired here.
  The `amber` path from `Notification` and the `error` path from
  `StopFailure` are therefore implemented against documentation only. A
  genuinely failed turn may not turn a row red in practice.
- Scan `status` values `shell` and `waiting` are listed by the CLI but not
  yet seen.
- Hook overhead with six or more concurrent sessions is unmeasured.
- What a user sees when a hook times out on Claude Code's side, and any
  hook behaviour outside `manual` permission mode, are unverified.

### 8.4 Tests

The Rust workspace runs 293 unit and integration tests on Windows,
concentrated in the state machine (53), registry (45), reveal (26), window
placement (24), persistence (24), supersession (17), and ingest (16). The
surface has 70 TypeScript tests.
An integration test drives six synthetic sessions through the real shim
and daemon pipeline, headless. A screenshot-level version of the
six-session colour test, run against live sessions, remains open Phase 1
work. One ingest test is known to fail intermittently and is tracked.

The code is small: about 8,000 lines of daemon Rust across 18 modules, a
101-line shim, and about 1,000 lines of surface TypeScript. The daemon
depends on `tauri`, `serde`, `tiny_http`, `getrandom`, and `windows-sys`.
The shim depends on nothing.

---

## 9. Adapters and Other Runtimes

Claude Code is the first runtime, reached through an adapter boundary
([ADR-003](DECISIONS.md#adr-003)).
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md) defines that boundary at
version 0 and says plainly that it will change once a second adapter
exists, because a contract with one implementation is a guess. Two
capabilities are required, observing status and listing sessions. The rest
are optional and declared per session rather than per adapter, because a
session's host decides what is possible: a session inside the VS Code
extension has no terminal window of its own to send keystrokes to, while a
terminal session does ([ADR-023](DECISIONS.md#adr-023)).

The protocol also distinguishes attached mode, watching a session the user
started, from hosted mode, where Deckhand would start sessions itself
through the Claude Agent SDK and gain full control at the cost of the
normal terminal interface ([ADR-004](DECISIONS.md#adr-004)). Only attached
mode exists. A plan for a second adapter, observing OpenAI Codex sessions
through its own hooks, is researched and proposed
([OPENAI_INTEGRATION_PLAN.md](OPENAI_INTEGRATION_PLAN.md)); no part of it
is implemented.

---

## 10. Limitations

- **One user.** Every observation of real use comes from the author's
  machine and working habits. There is no user study.
- **Windows only.** The no-focus-steal window, Reveal, the lifecycle, and
  Start with Windows are Win32. macOS and Linux are intended and unproven.
- **The premise is still being tested.** Whether hook-inferred status is
  reliable enough to trust at a glance is the question Phase 1 exists to
  answer. Three channels and a tiebreak make the board much harder to
  fool, but two of the twelve events have never been seen.
- **Semi-documented inputs.** The scan's `status` values changed between
  Claude Code versions. The VS Code lock files Reveal reads are an
  internal format that ADR-023 warns against building on, and ADR-032
  depends on them anyway, with a miss as the failure mode.
- **Supersession rests on process ancestry.** Hiding an older VS Code
  session relies on a shared parent process and folder, not on any signal
  that a conversation was closed, because none is known (section 4.6).
- **Partial Reveal.** Reveal cannot pick a tab inside Windows Terminal or a
  conversation inside the VS Code extension. It raises the window, or
  declines.
- **An accessibility gap.** Moving the window requires a drag
  (section 7.3).
- **Developer installation.** Deckhand builds from source, and hook repair
  assumes a checkout. There is no installer.

---

## 11. Roadmap

| Phase | Scope | State |
|---|---|---|
| 0 | Specification and feasibility spikes | Done, except observing two hook events |
| 1 | Observation that can be trusted at a glance | In progress |
| 2 | Approve and deny through the `PreToolUse` gate | Designed; control shape undecided |
| 3 | The full physical control set | Retired by ADR-028 |
| 4 | Hosted mode through the Claude Agent SDK | Not started |
| 5 | Push-to-talk | Retired by ADR-028 |
| 6 | Runtimes beyond Claude Code | OpenAI plan proposed |

Phase 1's exit criterion is several concurrent sessions across more than
one repository, watched reliably through every state including `error`,
with rows that prune themselves, and no manual correction. The open work
is in [TODO.md](../TODO.md): the live six-session colour test, hook
registration verified against a fresh install, closing same-user event
spoofing, and surfacing the watchdog ledger in the settings panel.

---

## 12. Conclusion

Deckhand's contribution is modest and specific. It is a status board for
concurrent coding-agent sessions that reads Claude Code's own signals,
reconciles them with two independent channels, and refuses to show a
colour it cannot justify. Its surface is a single column of rows that
costs one glance to read and one click to act on. It was built by
narrowing: a design that began as a full software macropad was cut back to
the part that earned its place, because an honest status light is the
precondition for everything that would act on a session.

The most transferable idea is the discipline around evidence, more than
any single mechanism. Every integration claim carries a stamp, every
channel has a stated failure mode, every failure resolves towards grey or
towards asking a human, and the approval path was designed in full and
then deliberately left unbuilt until the observation beneath it proves
itself. For users for whom checking on an agent is physically expensive,
that trust is the whole value.

---

## Appendix A: Decision Index

The decisions cited in this paper, from
[DECISIONS.md](DECISIONS.md). The record is append-only; a decision is
changed by a later entry, never by editing.

| ADR | Decision |
|---|---|
| 001 | Build a software reinterpretation of the Codex Micro |
| 002 | Tauri v2, Rust daemon, TypeScript surface |
| 003 | Claude Code first, through an adapter boundary |
| 004 | Attached mode before hosted mode |
| 005 | Hooks are the status source; transcripts are never load-bearing |
| 006 | The `PreToolUse` gate is the approval mechanism, and it fails to `ask` |
| 007 | Loopback HTTP with a token between shim and daemon |
| 008 | Keep the device's colour language, add `unknown`, never colour only |
| 013 | Amber carries a kind, and questions get answer targets |
| 014 | The default gate is narrow |
| 015 | The gating hook emits a decision and nothing else |
| 016 | Liveness by open-operation bracketing, not turn duration |
| 017 | `claude agents --json` is a second observation channel |
| 018 | Permission mode is a first-class axis |
| 019 | `complete` waits for live children |
| 020 | Attached-mode send is unproven, not impossible |
| 021 | No tooltip-only reveals |
| 022 | `default` is not a permission mode; `manual` is |
| 023 | The host is a third axis, and capabilities belong to a session |
| 024 | The scan recovers bindings, not state |
| 025 | Tauri clears the no-focus-steal bar on Windows |
| 026 | First live validation, and what reality corrected |
| 027 | A row click selects and raises |
| 028 | The surface narrows to a session list |
| 029 | Taller rows, header counts, a bundled typeface |
| 030 | A Hide grey toggle joins the header |
| 031 | Move removed, the header redrawn as a drag bar |
| 032 | Reveal classifies the host, and a tie is a miss |
| 033 | A gear-triggered settings panel joins the header |
| 034 | Visual refresh of the header, settings panel, and rows |
| 035 | Liveness from the process, state from the scan |
| 036 | The scan breaks ties when hooks fall silent; no transcript fallback |
| 037 | One instance, and a watchdog that restarts a crash |
| 038 | A newer session in the same VS Code window hides an idle older one |

## Appendix B: Glossary

- **Attached mode:** watching a Claude Code session the user started,
  through hooks and the scan.
- **Hook:** a command Claude Code runs at a defined moment in a session,
  passing a JSON payload on standard input.
- **Host:** what holds a session's process: a console, Windows Terminal,
  or the VS Code extension.
- **Reveal:** bringing the window that hosts a session to the front.
- **Scan:** the periodic run of `claude agents --json`.
- **Shim:** `deckhand-shim.exe`, the program registered as the hook
  command, which forwards each event to the daemon.
- **`T_unknown`:** the fifteen-minute silence after which a session
  without contrary evidence turns grey.
