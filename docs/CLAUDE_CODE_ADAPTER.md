# Claude Code adapter

Status: **proposed**. This is the reference adapter and the reason Deckhand
exists. It implements the contract in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md)
against Claude Code.

> **Verification stamp: partial, against Claude Code 2.1.220.** Checked on
> 2026-07-30 against the native single-binary build at
> `C:/Users/owenp/.local/bin/claude.exe`, on Windows 11. Exactly three
> things below are **observed**, meaning run or read on that machine:
>
> - `claude agents --json`, which returned the live sessions with `pid`,
>   `cwd`, `kind`, `startedAt`, `sessionId`, `name`, and `status`.
> - The status line payload keys, from a captured invocation.
> - The `~/.claude/projects/` directory mangling: the drive colon is
>   dropped and path separators collapse to single dashes. 41 project
>   directories, no index file.
>
> Everything else is **documented** (read from the public Claude Code
> documentation, not seen to fire here) or **unverified** (neither). Hook
> names, hook payload fields, and the permission decision vocabulary are
> `documented` at best, including the ones this file now names for the
> first time. Do not cite any of them as proven. Validating payloads,
> timings, and edge cases against a live install is still the first Phase 1
> task and still gates Phase 1 ([ADR-009](DECISIONS.md#adr-009)); this
> partial stamp does not close that item.

Named here so that nothing cites them by accident, these stay unverified:

- Hook overhead at six concurrent sessions.
- What the user sees when a hook times out on Claude Code's side.
- How conflicting decisions across two hook entries resolve.
- Any behaviour outside `manual` and `default` permission mode.

## The two modes, concretely

| | Attached | Hosted |
| --- | --- | --- |
| Who starts the session | You, in your own terminal | Deckhand, via the Claude Agent SDK |
| Status observation | Hooks | SDK message stream |
| Approve and deny | `PreToolUse` hook decision | SDK permission callback |
| Send a prompt | Turn-boundary channels only, none observed; `send_prompt` is `false` | Supported |
| Interrupt | Synthetic only, off by default | Supported |
| Terminal UI | Yours, untouched | None; the detail panel is the UI |
| Phase | 1 to 3 | 4 |

The painful cell is attached-mode send, and it is worth stating precisely,
because "impossible" and "unproven" are different claims and only the second
one is true.

What is documented: a `Stop` hook may return `decision: "block"` with a
reason, which puts that reason back in front of the model instead of letting
the turn end; `SessionStart` can supply an `initialUserMessage`; several
hooks can attach `additionalContext`. Those are real channels and they carry
text into a session. What none of them does is deliver into an *idle*
session at a time of the user's choosing, which is exactly when a person
wants to type. `claude -p --resume <id>` does not help either: it runs a new
turn in a new process rather than typing into the one on your screen.

So attached-mode send is not blocked by the absence of an interface, it is
blocked by the shape of the ones that exist, and none of them has been
observed here. `send_prompt` stays `false` in attached mode and Deckhand
ships no send UI on documentation alone. A Phase 1 spike observes Stop-hook
block behaviour on a live install: whether the held turn stays the same
session, what the user sees in the terminal while it is held, and what
ceiling exists on holding it open. If that spike lands, the capability is
reconsidered then, not before. Until it does, Send is disabled unless you
explicitly enable the synthetic keyboard fallback, and the button says what
it is.

## Interfaces used, and what they rest on

| Interface | Used for | Stability |
| --- | --- | --- |
| Hooks (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `SubagentStart`, `SubagentStop`, `Notification`, `Stop`, `StopFailure`, `PermissionDenied`, `SessionEnd`) | Status, approvals | Documented |
| `PreToolUse` permission decision output | Approve and deny | Documented |
| Hook payload fields `session_id`, `cwd`, `transcript_path`, and where present `prompt_id`, `permission_mode`, `effort.level`, `tool_use_id` | Session identity, mode badge | Documented |
| `claude agents --json` | Cold-start enumeration, host `pid` for Reveal | Documented, keys observed 2.1.220 |
| Status line JSON | Tile extras | Documented, keys observed 2.1.220, schema may grow |
| Agent SDK (`@anthropic-ai/claude-agent-sdk`) | Hosted mode | Documented |
| Enumerating `~/.claude/projects/` | Populating the bind picker | **Internal**, mangling observed 2.1.220 |
| Transcript JSONL per-line schema | Last-resort detail | **Internal, changes between releases** |
| Window-title heuristics to find a session's terminal | Fallback host match when no `pid` is available, synthetic input | **Synthetic** |

The design rule that follows: the `documented` rows may be load-bearing, the
`internal` and `synthetic` rows may not. If every `internal` row broke
tomorrow, Deckhand should degrade (the bind picker goes empty and you bind by
hand) rather than fail. Note the direction of travel in the bottom two rows:
`claude agents --json` reports a `pid` per session, so host matching prefers
that `pid` and falls back to window titles only when it has none.

`claude agents --json` is the one row that survives with hooks switched off,
which is why it now carries cold start instead of `~/.claude/projects/`.
The hooks row names twelve events out of the thirty-plus that Claude Code
documents; why it stops at twelve is under
[Hook installation](#hook-installation).

## Status inference

The daemon owns the state machine; this adapter feeds it observations.

| Observation | Reported state | Notes |
| --- | --- | --- |
| `SessionStart`, any `source` except `compact` | `idle` | Also registers the session and its `cwd` |
| `SessionStart`, `source: "compact"` | no change | Auto-compaction fires mid-turn; mapping it to `idle` would flip a live blue tile white |
| `UserPromptSubmit` | `thinking` | A turn is beginning |
| `PreToolUse`, tool `AskUserQuestion` | `needs_input` | `kind: "question"`. The question and its options are read from the tool input. This is the amber that Answer exists for |
| `PreToolUse` held by Deckhand's gate | `needs_input` | `kind: "permission"`. This is the amber that Approve answers |
| `PreToolUse`, any other tool, no gate | `thinking` | Detail: tool name |
| `PermissionRequest` | `needs_input` | `kind: "permission"`. Observation only, and not installed. See the note below |
| `PermissionDenied` | `thinking` | Something other than Deckhand denied the call. See the note below on how little it says |
| `Notification`, `notification_type: "agent_needs_input"` | `needs_input` | No `kind`: the payload does not say which sort of prompt it is. The field is `notification_type`, not `type` |
| `Notification`, `notification_type: "agent_completed"` | `idle` | |
| `PostToolUse` | `thinking` | Success only. Closes the operation |
| `PostToolUseFailure` | `thinking` | The turn continues. Detail `error: { kind, message? }`, `kind` from this event's `error_type`. Closes the operation |
| `Stop` | `complete` | Cleared to `idle` when you select the tile |
| `StopFailure` | `error` | An API error ended the turn. The typed field is `error`, not `error_type` |
| `SubagentStart`, `SubagentStop` | no change | Children never move the parent's colour. They feed the child ledger and the liveness bracket, and nothing else |
| `SessionEnd`, `reason` of `clear` or `resume` | no change | Each is followed by a new `SessionStart` for the same terminal |
| `SessionEnd`, any other `reason` | `ended` | |
| No events past `T_unknown` | `unknown` | Never guessed into `idle` or `error` |
| Process confirmed dead without `SessionEnd` | `error` | A crash; detection method on Windows still open |

The "no change" rows earn their place. A `SessionStart` whose source is
`compact`, and a `SessionEnd` whose reason is `clear`, both fire in the
middle of ordinary work, and the unqualified mappings this table used to
carry would have repainted a tile whose session had not changed state at
all. Auto-compaction and `/clear` are daily events, so both were daily bugs.

**Red now has a mechanism, but not an observation.** `StopFailure` is
documented to fire when an API error ends the turn, carrying a typed `error`
field, and it is fire-and-forget. That is the signal this file previously
said did not exist. It is `documented`, not `observed`: nobody here has seen
it fire, and the local corpus is a warning against a naive mapping, since 66
of 71 transcript `api_error` records are retries that recover. So red still
means "the turn failed or the session died", the spec promises no more than
that, and the open question and the TODO spike both stay open. A string in a
binary is not an observation.

**`PermissionDenied` says less than its name suggests.** Since 2.1.208 it
carries `reason` (not `denial_reason`), and that reason is usually the fixed
string "Blocked by classifier". Treat it as "a call was refused by something
that is not you", which is enough to keep the tile blue and to make the
second gate visible, and not enough for anything else.

**`AskUserQuestion` is why `PreToolUse` is watched twice.** It is an ordinary
tool call, so the narrow gate never sees it. The non-gating `matcher: "*"`
entry does, which is what makes `kind: "question"` a state this adapter can
actually report rather than one the surface waits for forever. Nothing is
held: the entry is `async`, the question is reported, and the answer channel
is a separate matter that stays unproven, so the Answer targets render
disabled until it is not.

Every `PreToolUse` opens an operation for the liveness rule, whichever of the
three rows it lands on, and the operation closes the same way in all three.
The rows are read most specific first: a gated call fires both entries, and
the gated row wins, because `thinking` on a call that is waiting for a human
is the wrong colour and the one the gate exists to replace.

**An amber can arrive without a kind, and that is not a failure.**
`Notification` says a prompt is on screen and nothing about which sort. The
adapter reports `needs_input` with no `kind` rather than guessing one, and
the surface enables neither Approve nor Answer on it, because both would be
buttons that cannot do what their labels say. See
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types).

**`PermissionRequest` is not a candidate for the gate.** It has no documented
`ask` decision, so it can observe a request but cannot hold one. `PreToolUse`
stays the gate and this event is observation only. That question is closed,
not spiked. It is in the table because the state machine has an answer ready
for it, not because Deckhand installs it today; see
[Hook installation](#hook-installation) for why the installed set stops where
it does.

## Hook installation

Deckhand needs a block like this in the user-level settings file:

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "PreToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] },
      { "matcher": "Bash", "if": "Bash(rm:*)", "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 120 }] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "PostToolUseFailure": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "SubagentStart": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "SubagentStop": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "PermissionDenied": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "StopFailure": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "async": true }] }
    ]
  }
}
```

Thirteen entries across twelve events: the seven Phase 1 events plus the five
the state machine now depends on (`PostToolUseFailure`, `StopFailure`,
`PermissionDenied`, `SubagentStart`, and `SubagentStop`), with `PreToolUse`
carrying two entries for the reason below. It stops there on purpose.
Thirty-plus events are documented, and adding them is cheap to write and
expensive to be wrong about, so the rest wait behind the hook overhead
measurement rather than arriving on the argument that they exist.

`SubagentStart` and `SubagentStop` are installed because two rules read
them, not because subagents are interesting on their own: they open and
close the child ledger that holds `COMPLETE` back
([ADR-019](DECISIONS.md#adr-019)), and they are one of the two brackets the
liveness rule counts ([ADR-016](DECISIONS.md#adr-016)). Without them the
ledger is permanently empty and both rules are dead letters.

**`PreToolUse` carries two entries, and only one of them gates.** The
gating entry is deliberately narrow: the shipped default is a small set of
entries scoped to shell execution and file deletion, of which the one above
is an example, and never `matcher: "*"`. The `if` field takes
permission-rule syntax; that syntax is `documented` and unverified here,
which is one more reason the installer shows the exact block and waits for a
yes before writing anything. The second entry is `matcher: "*"` and
`"async": true`, so it decides nothing and nothing waits on it. It exists
because two rules need to see every tool call, not just the gated ones:
the liveness bracket has no other way to open ([ADR-016](DECISIONS.md#adr-016)),
and `AskUserQuestion` is an ordinary tool call, so a narrow gate would leave
the amber that Answer exists for unobservable. Rule 5 below counts gating
entries, not entries, which is what makes the pair legal.

Rules for how Deckhand handles that block:

1. **Deckhand never edits `settings.json` silently.** It shows the exact block,
   explains what each entry does, and applies it only on explicit confirmation.
   Editing another tool's configuration is an intrusive act even when helpful.
2. **User level only.** Deckhand writes to the user-level
   `~/.claude/settings.json` and never to a project's `.claude/settings.json`.
   The project file is usually git-shared, and committing a machine-local shim
   path would push one machine's paths onto everyone else's checkout. The
   `hooks` key may be absent from the file entirely, which is a normal state
   and not an error.
3. **Compose, never clobber.** Hook entries are arrays precisely so multiple
   tools can coexist. Deckhand appends its entries and removes only its own.
4. **Uninstall must be complete.** Removing Deckhand removes every entry it
   added, verified, because an orphaned `PreToolUse` hook pointing at a dead
   shim is at best latency on every tool call.
5. **The health check counts Deckhand's entries.** Exactly one gating entry
   per event, and it reports duplicates loudly. Two shim entries, whether from
   two installs or one install plus an orphan, can return two different
   decisions for one tool call, and how Claude Code resolves that is
   undocumented.
6. **Status hooks are `async`, the gate never is.** Every non-gating entry
   carries `"async": true`: nothing waits on it and nothing it prints is read.
   The gate is synchronous, because the output of an async hook is ignored,
   and an ignored permission decision is precisely the failure
   [ADR-006](DECISIONS.md#adr-006) exists to prevent. The gate is also the only
   entry allowed a long `timeout`, and only when gating is on.
7. **No `timeout` on `SessionEnd`.** Those hooks share a budget of about 1.5
   seconds, and a longer per-hook timeout raises the ceiling rather than
   lowering it, so a value there slows every session exit for no gain.

Windows notes: hook commands run through Git Bash when present, with a
PowerShell fallback; paths in the JSON use forward slashes. The shim is a small
native binary so that neither shell dialect matters beyond launching it.
Documented but not adopted: hooks may have `"type": "http"`, which would
delete both the shim and the shell-dialect problem in one move. It is an open
question, not a decision, because it moves the approval path onto a network
listener and that needs its own pass over
[SECURITY_MODEL.md](SECURITY_MODEL.md).

## The approval path, precisely

When gating is enabled for a session, the `PreToolUse` shim call does not
return immediately. The daemon holds it while the tile turns amber and the
detail panel shows the tool name and its input.

**A gated amber means your pattern matched, not that Claude Code would have
asked you.** `PreToolUse` fires on every matching tool call, whatever the
session's permission mode, so the gate manufactures its own ambers. That is
the whole reason the default gate is narrow. Point a match-all gate at a
session in `auto` mode and a run that would have interrupted nobody becomes
one amber per tool call, with Deckhand as the sole cause of the clicks it
exists to remove. [SECURITY_MODEL.md](SECURITY_MODEL.md) states the same
rule from the security side.

The exit paths:

- **Allow or Deny clicked.** The shim prints exactly these three fields, then
  exits:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Approved on Deckhand"
  }
}
```

- **Deckhand's deadline expires** (daemon answers well inside the hook's own
  `timeout`): the decision is `"ask"`, which hands the prompt back to Claude
  Code's own UI. Nothing is silently allowed and nothing the user wanted is
  silently denied. This is what failing closed means here.
- **The daemon is unreachable.** The shim exits immediately with no decision
  output, and Claude Code behaves exactly as if Deckhand did not exist. A dead
  companion must never brick every session on the machine.

A fourth decision value, `defer`, is documented alongside `allow`, `deny`,
and `ask`. Deckhand does not use it anywhere, and in particular does not use
it for either fallback above. `defer` falls through to the normal permission
flow, and what that flow is depends on the mode: the classifier in `auto`,
execution in `bypassPermissions`. `ask` is the only value that reliably
lands the decision somewhere safe, so `ask` stays the fail-closed exit and
[ADR-006](DECISIONS.md#adr-006) is unchanged.

**What `ask` reaches depends on the session's permission mode.** In
`default` and `manual` it returns the decision to a human at the terminal,
which is the case this design was written against. In `auto` it returns the
decision to Claude Code's classifier, which usually answers it without a
person seeing anything. In `dontAsk` it resolves to a denial. The gate is
not dead in any of them: `PreToolUse` still runs first in every mode, and a
hook `allow` still runs the call in `dontAsk`. The adapter therefore reports
`permission_mode` from the hook payload into `SessionInfo`, and reports
`unknown` when a payload does not carry it, because a mode the tile guessed
is worse than a mode it admits it does not know.

Gating is **per session and off by default**. Turning it on is a security
decision and is treated as one; see [SECURITY_MODEL.md](SECURITY_MODEL.md).

## Extras from the status line

Claude Code passes a `statusLine` command a JSON blob including context-window
usage and cost. Deckhand can install a status line script that forwards that
JSON to the daemon, which gives tiles a context-remaining ring and a cost
figure at no extra process cost beyond what the status line already spends.
Optional, documented interface, and purely additive.

The keys, from a payload captured on this machine (`observed`, 2.1.220), are
more specific than "context and cost": `context_window.used_percentage`,
`cost.total_cost_usd`, `cost.total_lines_added`, `cost.total_lines_removed`,
and `rate_limits.{five_hour,seven_day}.{used_percentage,resets_at}`. The
rate limit keys are the interesting new ones, since a session that is about
to hit a five-hour limit is a thing a status board could honestly show. The
schema may grow, so the forwarder passes the blob through and the surface
reads the keys it knows.

## Cold start and rebinding

Hooks only tell you about things that happen after they are installed, so a
daemon starting while sessions are already running used to know nothing about
any of them until each one next emitted an event. In practice that meant a
board of grey tiles after every restart, which is a daily event, not an edge
case.

`claude agents --json` closes most of that gap. It is documented, it needs no
TTY, and on this machine it returned the live sessions with `pid`, `cwd`,
`kind`, `startedAt`, `sessionId`, `name`, and `status` (`observed`, 2.1.220).
Cold start, in order:

1. Enumerate live sessions with `claude agents --json` and rebind tiles by
   `session_id`.
2. Map the reported `status`. `busy` becomes `thinking`. **Everything else,
   including a status that is missing or a value this adapter does not
   recognise, becomes `unknown`.** Never guess `idle`: white is a claim that
   a session is sitting waiting for you, and this channel does not support
   that claim.
3. A tile whose binding no enumeration explains stays `unknown` until an
   event arrives for it. That is the correct answer, not a failure.
4. `~/.claude/projects/` is read only to populate the bind picker with recent
   sessions. It is the `internal` interface: it names things, it never infers
   state, and if the mangling changes tomorrow the only casualty is a
   convenience list.

The `pid` from the same call is what Reveal should match a host window on,
in preference to window-title heuristics, since a title is a guess and a
`pid` is not.

This supersedes part of [ADR-005](DECISIONS.md#adr-005), which took hooks to
be the only status source. Hooks remain the only source of state *changes*.
Enumeration is an inventory: it tells you a session exists and, for one
value, that it is busy. The superseding ADR records the split. ADR-005 itself
is unedited, as ADRs always are.

## Hosted mode sketch

Phase 4. The daemon links the Agent SDK (TypeScript package
`@anthropic-ai/claude-agent-sdk`; Python exists too) and starts sessions it
fully owns: prompts in, streamed messages out, permission callbacks in-process
instead of via hooks. SDK sessions persist under the same `~/.claude/projects/`
tree, so a hosted session can later be resumed from a terminal. The open design
question is the detail panel: hosted mode makes Deckhand the only UI for that
session, and a status board that grows a transcript viewer is a much bigger
thing than a status board.

## Capability declaration

What this adapter will declare, per [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md):

| Capability | Attached | Hosted |
| --- | --- | --- |
| `observe_status` | `documented` | `documented` |
| `list_sessions` | `documented` (observed 2.1.220) | `documented` |
| `focus_session` | `synthetic` | `false` (no window to raise) |
| `decide_permission` | `documented` | `documented` |
| `answer_question` | `false` (optional, and unproven) | `false` (unproven) |
| `send_prompt` | `false` (`synthetic` if the user opts in) | `documented` |
| `interrupt` | `false` (`synthetic` if the user opts in) | `documented` |
| `set_option` | `false` | `documented` |

Three of those rows need a sentence.

`list_sessions` is the one promotion here, and it is earned. The command
`claude agents --json` was run on this machine and returned live sessions,
which makes it the only row in the table carrying an observation at all.

`answer_question` is optional and unproven, and it is declared so that the
surface has a real thing to disable an Answer target against rather than
inventing one. `AskUserQuestion` is an ordinary tool call, so its options are
visible in a `PreToolUse` payload and Deckhand installs the entry that sees
them, but no interface for returning a chosen option to a running session has
been observed, and it is not promoted on anything less. Deckhand can
therefore show the question and cannot yet answer it, which is the honest
half of the feature and is what ships. MCP elicitation is sometimes suggested
here; it is MCP-only, so it answers a different question and is not a
candidate.

`set_option` is `false` in attached mode because no local interface sets the
model, the effort level, or the permission mode of a session that is already
running. The dial is a readout there, and the surface should say so rather
than offering a control with nowhere to write.

## Known limitations

1. No local, arbitrary-time prompt injection into a running interactive
   session. The documented channels (Stop-hook `decision: "block"`,
   `SessionStart` `initialUserMessage`, `additionalContext`) all deliver at a
   turn boundary and none of them has been observed here, so `send_prompt`
   stays `false` in attached mode.
   See [the two modes](#the-two-modes-concretely) for the shape of each one.
2. Error detection now has a documented mechanism (`StopFailure`) but still
   no observation, so red stays a narrow promise.
3. `--resume` semantics need testing: whether a resumed session keeps its id in
   hook payloads, and what a bound tile should do when a session forks.
4. **Deckhand is not the only gate.** In `auto` mode a classifier answers
   most permission prompts before any human sees them, and `auto` is the
   default on the machine this design was written on. `PermissionDenied` is
   what makes that second gate visible at all. An earlier version of this
   file claimed Deckhand could assume it was the only gate; that was wrong.
5. Two Deckhands, or one Deckhand plus an orphaned shim entry left by an
   incomplete uninstall, can return two different decisions for one
   `PreToolUse` call, and how Claude Code resolves that is undocumented. The
   settings health check must detect duplicate entries and say so plainly
   rather than leaving the user with a gate whose behaviour nobody can
   predict.
6. `disableAllHooks: true`, `--safe-mode`, and `--bare` each switch off hooks
   **and** the status line together, so a session running under any of them
   is invisible to both of Deckhand's main channels. `claude agents --json`
   survives all three, but has its own off switch, `disableAgentView`. A tile
   in that state must say "hooks are disabled" and mean it, not sit silently
   grey while looking exactly like a dead session.
7. Most of this file is documented rather than observed. See the stamp at the
   top for what is not.
