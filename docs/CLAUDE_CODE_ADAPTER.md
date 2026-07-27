# Claude Code adapter

Status: **proposed**. This is the reference adapter and the reason Deckhand
exists. It implements the contract in [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md)
against Claude Code.

> **Verification stamp: none yet.** Everything below was written from the
> public Claude Code documentation, not from a running integration. Validating
> the payloads, timings, and edge cases against a live install is the first
> Phase 1 task, and this file gets a "verified against version X" line at the
> top when that happens. Until then, treat details as probable, not proven.

## The two modes, concretely

| | Attached | Hosted |
| --- | --- | --- |
| Who starts the session | You, in your own terminal | Deckhand, via the Claude Agent SDK |
| Status observation | Hooks | SDK message stream |
| Approve and deny | `PreToolUse` hook decision | SDK permission callback |
| Send a prompt | **Not supported by any documented interface** | Supported |
| Interrupt | Synthetic only, off by default | Supported |
| Terminal UI | Yours, untouched | None; the detail panel is the UI |
| Phase | 1 to 3 | 4 |

The painful cell is attached-mode send. Claude Code has no supported way for an
external process to put a prompt into a running interactive session, and
`claude -p --resume <id>` runs a new turn in a new process rather than typing
into the one on your screen. Deckhand does not pretend otherwise. In attached
mode, Send is disabled unless you explicitly enable the synthetic keyboard
fallback, and the button says what it is.

## Interfaces used, and what they rest on

| Interface | Used for | Stability |
| --- | --- | --- |
| Hooks (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Notification`, `Stop`, `SessionEnd`) | Status, approvals | Documented |
| `PreToolUse` permission decision output | Approve and deny | Documented |
| Hook payload fields `session_id`, `cwd`, `transcript_path` | Session identity | Documented |
| Status line JSON (context %, cost) | Tile extras | Documented, schema may grow |
| Agent SDK (`@anthropic-ai/claude-agent-sdk`) | Hosted mode | Documented |
| Enumerating `~/.claude/projects/` on cold start | Rebinding tiles after a daemon restart | **Internal** |
| Transcript JSONL per-line schema | Last-resort detail | **Internal, changes between releases** |
| Window-title heuristics to find a session's terminal | `focus_session`, synthetic input | **Synthetic** |

The design rule that follows: the top five rows may be load-bearing, the bottom
three may not. If every `internal` row broke tomorrow, Deckhand should degrade
(cold starts show unbound tiles until sessions emit events) rather than fail.

## Status inference

The daemon owns the state machine; this adapter feeds it observations.

| Observation | Reported state | Notes |
| --- | --- | --- |
| `SessionStart` | `idle` | Also registers the session and its `cwd` |
| `UserPromptSubmit` | `thinking` | A turn is beginning |
| `PreToolUse` received, no gate | `thinking` | Detail: tool name |
| `PreToolUse` held by Deckhand's gate | `needs_input` | This is the amber that Approve answers |
| `Notification` (permission prompt) | `needs_input` | Claude Code's own prompt is on screen |
| `Notification` (idle prompt) | `idle` | |
| `PostToolUse` | `thinking` | Turn still running |
| `Stop` | `complete` | Cleared to `idle` when you select the tile |
| `SessionEnd` | `ended` | |
| No events past the liveness deadline | `unknown` | Never guessed into `idle` or `error` |
| Process confirmed dead without `SessionEnd` | `error` | A crash; detection method on Windows still open |

**Error is the honest gap.** There is no clearly documented hook that fires on
a failed turn. Until a reliable signal is proven, red means "the session
crashed", and the spec does not promise more. If a
failed-turn signal turns out to exist, red gets richer and this table changes.

## Hook installation

Deckhand needs a block like this in Claude Code settings:

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ],
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ],
    "PreToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 120 }] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "deckhand-shim", "timeout": 5 }] }
    ]
  }
}
```

Rules for how Deckhand handles that block:

1. **Deckhand never edits `settings.json` silently.** It shows the exact block,
   explains what each entry does, and applies it only on explicit confirmation.
   Editing another tool's configuration is an intrusive act even when helpful.
2. **Compose, never clobber.** Hook entries are arrays precisely so multiple
   tools can coexist. Deckhand appends its entries and removes only its own.
3. **Uninstall must be complete.** Removing Deckhand removes every entry it
   added, verified, because an orphaned `PreToolUse` hook pointing at a dead
   shim is at best latency on every tool call.
4. Status hooks are cheap and non-blocking with short timeouts. Only the
   `PreToolUse` gate is allowed a long timeout, and only when gating is on.

Windows notes: hook commands run through Git Bash when present, with a
PowerShell fallback; paths in the JSON use forward slashes. The shim is a small
native binary so that neither shell dialect matters beyond launching it.

## The approval path, precisely

When gating is enabled for a session, the `PreToolUse` shim call does not
return immediately. The daemon holds it while the tile turns amber and the
detail panel shows the tool name and its input. The exit paths:

- **Allow or Deny clicked.** The shim prints, then exits:

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

Gating is **per session and off by default**. Turning it on is a security
decision and is treated as one; see [SECURITY_MODEL.md](SECURITY_MODEL.md).

## Extras from the status line

Claude Code passes a `statusLine` command a JSON blob including context-window
usage and cost. Deckhand can install a status line script that forwards that
JSON to the daemon, which gives tiles a context-remaining ring and a cost
figure at no extra process cost beyond what the status line already spends.
Optional, documented interface, and purely additive.

## Cold start and rebinding

Hooks only tell you about things that happen after they are installed. When the
daemon starts while sessions are already running, it knows nothing until each
session next emits an event. The honest behaviours, in order of preference:

1. Tiles restore their bindings by `session_id` but show `unknown` until an
   event arrives.
2. Optionally, the adapter enumerates `~/.claude/projects/` to offer a list of
   recent sessions for binding. That is the `internal` interface, used only to
   populate a picker, never to infer state.

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
| `list_sessions` | `internal` (cold start) | `documented` |
| `focus_session` | `synthetic` | `false` (no window to raise) |
| `decide_permission` | `documented` | `documented` |
| `send_prompt` | `false` (`synthetic` if the user opts in) | `documented` |
| `interrupt` | `false` (`synthetic` if the user opts in) | `documented` |
| `set_option` | `false` | `documented` |

## Known limitations

1. No prompt injection into a running interactive session, by design of
   Claude Code, not of Deckhand.
2. Error detection is unproven; red is currently a narrow promise.
3. `--resume` semantics need testing: whether a resumed session keeps its id in
   hook payloads, and what a bound tile should do when a session forks.
4. Two observers are fine (hooks fan out), but two *controllers* are not a
   supported concept; Deckhand assumes it is the only gate on a session.
5. Everything in this file is unverified against a live install. See the stamp
   at the top.
