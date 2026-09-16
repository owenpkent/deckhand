# Upstream asks

Status: **proposed**. This is a list of things Deckhand needs from the
runtimes it observes, written so each ask can be filed as an issue without
being re-derived. It is authoritative for the ask list and for whether an
ask has been filed, and for nothing else: every claim about Deckhand's own
behaviour defers to the files in [WORKFLOW.md](WORKFLOW.md) section 1, and
every claim about a runtime defers to that runtime's adapter document.

> **Verification stamp: partial, against Claude Code 2.1.273 and against
> the `openai/codex` tree at commit `a6d4741`, both on 2026-09-16, on
> Windows 11.** The Claude Code items marked **observed** below were run
> or read on this machine against the native single-binary build at
> `C:/Users/owenp/.local/bin/claude`. The Codex items marked **observed**
> were read from the public repository at that commit, not run. Anything
> marked **documented** is taken from vendor prose and has not been
> reproduced here. Nothing in this file upgrades the stamp on
> [CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md), which remains against
> 2.1.220 and 2.1.270 for its own items.

---

## 1. Why this document exists

Deckhand carries a fair amount of machinery whose only purpose is to
recover information the runtime already has and does not hand over. A
Toolhelp32 process-tree walk, a per-session `OpenProcess` handle polled on
a two second tick, a read of an undocumented lock file, and a
window-title substring match are all standing in for facts the runtime
knows exactly and Deckhand can only infer.

Each of those workarounds is a real cost: it is code to maintain, it is a
source of wrong answers, and several of them are the direct cause of an
ADR. Listing them in one place makes it possible to ask for the right
thing, and to notice when an ask has been answered.

This file is the evidence. The issues filed upstream are derived from it,
not written fresh.

## 2. Why none of these is a pull request

Neither runtime accepts external code contributions, for different
reasons, and both were checked on 2026-09-16.

**Anthropic.** `anthropics/claude-code` does not contain the CLI. Its tree
is plugins, examples, scripts, a devcontainer, and the changelog; there is
no `src/` and no `package.json` for the tool itself. `LICENSE.md` reads in
full: "© Anthropic PBC. All rights reserved. Use is subject to Anthropic's
Commercial Terms of Service." Recent merged pull requests land in `mods/`,
`plugins/`, `examples/`, or a skill file. **Observed.**

**OpenAI.** `openai/codex` is the real source, in Rust, but
`docs/contributing.md` states in bold: "We do not accept external code
contributions or pull requests." It directs community effort to issues,
repros, root-cause analysis, and feature requests. **Observed.**

So the deliverable for both is a well-evidenced issue. This is not a
limitation worth fighting; an issue carrying reproducible observations is
the strongest thing either tracker accepts.

## 3. The ask both runtimes share: host identity

This is the single most valuable ask, it applies to both runtimes, and
neither one answers it today.

**Need.** When the owner clicks a session row, Deckhand raises that
session's window. To do that it must know which window the session lives
in.

**Today.** Deckhand classifies the host by walking up to eight hops of the
process tree from the session pid, using a Toolhelp32 snapshot
(`app/src-tauri/src/host.rs`, `Host::{Vscode, WindowsTerminal, Console}`).
For a plain console it uses `AttachConsole` to get an exact window handle.
For VS Code it reads `~/.claude/ide/*.lock`, which is undocumented. When
none of that resolves, it falls back to matching window-title substrings,
and a tied score is treated as a miss rather than a guess
(`app/src-tauri/src/reveal.rs`). Windows Terminal tabs cannot be targeted
at all, which is blocked upstream of both vendors by
`microsoft/terminal#19783`, closed as not planned.

**Ask.** Persist the host axis in the session record: the terminal pid,
the tty or console handle where one exists, and an IDE window identifier
where the session was started by an extension. A window handle would be
ideal and an identifier that can be resolved to one is enough.

## 4. Asks for Anthropic, Claude Code

### 4.1 Expose in `claude agents --json` the fields already written to disk

This is the smallest ask on the list and probably the highest value per
word, because it asks for no new behaviour at all.

**Observed.** On 2.1.273 the CLI writes one record per session to
`~/.claude/sessions/<pid>.json`. A live record read on 2026-09-16 carried:

    pid, sessionId, cwd, startedAt, procStart, version,
    peerProtocol, peerFeatures, kind, entrypoint, pidDomain,
    messagingSocketPath, name, nameSource, nameSince,
    status, updatedAt, statusUpdatedAt

**Observed.** The supported read path, `claude agents --json`, returns a
strict subset: `pid`, `cwd`, `kind`, `startedAt`, `sessionId`, `name`, and
`status`. It drops four fields Deckhand needs and currently reconstructs:

| Dropped field | What Deckhand does instead |
| --- | --- |
| `entrypoint`, e.g. `claude-vscode` | the process-tree walk in section 3 |
| `procStart` | nothing; pid reuse is unguarded in handle polling |
| `nameSource`, e.g. `derived` | re-derives it, per ADR-038 |
| `messagingSocketPath` | see 4.2 |

**Ask.** Add `entrypoint`, `procStart`, and `nameSource` to
`claude agents --json`. Reading the on-disk record directly is not a
substitute: it is undocumented, and Deckhand should not build on it
without it being supported.

Note that `nameSource` is the clearest case. ADR-038 gave Deckhand a
`derived` flag so a row could tell a real session name from a directory
name. The CLI already tracks exactly that distinction and does not pass it
on.

### 4.2 Document the per-session peer channel

**Observed.** The same record carries `messagingSocketPath`, a per-session
named pipe, and `peerProtocol: 1` with
`peerFeatures: ["notify_idle", "artifact_yield"]`. Alongside each record
is a `<pid>.<hash>.key` file, which suggests the channel is authenticated.

**Not observed.** Nothing about what that protocol accepts or emits.
Deckhand has not connected to it and this document does not claim it can
be used.

**Ask.** Document the peer protocol, its authentication, and its stability
guarantees, or state that it is private and not to be built on. A feature
advertised as `notify_idle` is, on its name alone, close to the signal
section 4.3 asks for, and it would be better to use the channel that
exists than to ask for a second one.

### 4.3 A waiting-on-human event: mostly answered, and our gap to close

This started as an ask and turned out to be a Deckhand bug. It is kept
here because the correction is worth more than the ask was.

**Need.** The amber `needs_input` state is the one state that justifies an
always-on-top surface. Everything else can wait until the owner looks.

**What we believed.** That no immediate waiting-on-human signal existed,
because `Notification` has never been observed to fire here across months
of dogfooding, and `StopFailure` has not either.

**Observed, 2026-09-16.** `anthropics/claude-code#13024` requested exactly
this event and was closed as **completed** on 2026-08-17. The maintainer's
closing comment states that a `PermissionRequest` hook fires immediately
when Claude asks for permission, that `PreToolUse` with matcher
`AskUserQuestion` fires the moment the question is raised, and that the
matcher `AskUserQuestion|ExitPlanMode` also covers plan approval. It also
gives the reason `Notification` looked dead: `Notification` with
`permission_prompt` waits roughly six seconds of inactivity first, so a
surface that answers sooner never sees it.

**Our gap.** Deckhand registers twelve events and `PermissionRequest` is
not among them; it registers `PermissionDenied` instead
(`app/src-tauri/src/hook_status.rs`). Deckhand therefore manufactures
amber from a `PreToolUse` payload it holds itself rather than reading the
event built for the purpose. Adopting `PermissionRequest` is Phase 1 work
and belongs in [TODO.md](../TODO.md), not on a vendor's tracker.

**What is left to ask.** Much less than we thought, and possibly nothing.
`PermissionRequest` should appear in the published hooks reference, and
the six second `Notification` delay should be documented rather than
discovered by a maintainer comment on a closed issue. Neither is worth an
issue on its own. The residual observation gap is `auto` mode, whose
prompts route to a classifier Deckhand cannot see, and that is worth
filing only once we have tried `PermissionRequest` and can say precisely
what it still misses.

**Prior art, kept for the remaining asks.** Codex models session blocking
explicitly. `ThreadStatus::Active` carries `ThreadActiveFlag`, whose
variants are `WaitingOnApproval` and `WaitingOnUserInput`, pushed to
clients as `thread/status/changed`
(`codex-rs/app-server-protocol/src/protocol/v2/thread.rs:1663`).
**Observed** in the repository at `a6d4741`.

**Lesson for the rest of this file.** One ask here was stale by a month
and we did not know, because the answer arrived as a comment on a closed
issue rather than in the documentation. Re-check each remaining ask
against the current release immediately before filing it.

### 4.4 `StopFailure`, or documentation saying why it will not fire

**Need.** The red `error` state.

**Today.** `StopFailure` is documented and has never fired here. Red is a
state Deckhand can paint and has no trustworthy way to reach, which is
recorded as an open question in [ARCHITECTURE.md](ARCHITECTURE.md).

**Ask.** Either make it fire on a failed turn, or document the conditions
under which it does, so the adapter can stop promising a state it cannot
observe. Removing the state is the fallback, and it is a worse product.

### 4.5 An end-of-life signal that survives a kill

**Need.** Knowing a session is gone.

**Today.** `SessionEnd` does not fire when the process is killed or the
terminal closes. Deckhand holds a `PROCESS_SYNCHRONIZE` handle per session
and polls it with a zero-timeout `WaitForSingleObject` on the existing two
second tick (`app/src-tauri/src/liveness.rs`), which is ADR-035 in full.
ADR-036's tie-break scan and ADR-037's watchdog sit downstream of the same
gap.

**Ask.** A liveness primitive an external observer can watch without
polling. Codex solves this with an advisory file lock per thread whose
release is the death signal, which is a cheap pattern and does not require
the dying process to cooperate.

### 4.6 An answer channel for `AskUserQuestion`

**Need.** Deckhand can already read a question's text and its options from
the `PreToolUse` payload. It cannot return the chosen option, so
`answer_question` is `false` and unproven on every host
([ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md) lines 42 to 48).

**Ask.** A documented way to answer a pending question, keyed by request
id and option id, so that duplicate labels, multiple questions, and
free-text-only requests stay distinguishable. Answering must be idempotent
and must lose cleanly to another client answering first.

This is Phase 2 work and is not to be wired early. It is listed here
because the ask should be made before the phase starts, not during it.

### 4.7 A hook registration API

**Need.** Installing Deckhand's hooks without editing the owner's settings
by hand.

**Today.** Registration is a raw `settings.json` text edit. Two gating
entries, whether from two installs or from an orphan left by an uninstall,
can return conflicting decisions, and how Claude Code resolves that is
undocumented ([CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md), known
limitation 5).

**Ask.** A supported registration command, and a documented resolution
order when several hooks answer the same event. The security rule that a
deny always wins should be stated, not inferred.

## 5. Asks for OpenAI, Codex CLI

Codex already ships most of what section 4 asks Anthropic for. The asks
here are correspondingly narrow.

### 5.1 Host identity in the session record

**Observed.** `SessionMeta` carries `cwd`, `originator`, `cli_version`,
and a source field, with no pid, tty, or IDE window. A
`codex-rs/terminal-detection` crate exists and feeds the user agent rather
than the rollout.

**Ask.** Section 3, applied here: persist the host axis in `SessionMeta`.
This is the one gap that blocks click-to-raise on Codex, and it is the
issue worth filing first.

### 5.2 Third-party attach to a session the user started

**Need.** Deckhand observes sessions the owner started in their own
terminal. A protocol that only serves sessions the observer spawned does
not fit the product.

**Observed.** `codex app-server` speaks JSON-RPC, and
`codex-app-server-daemon` runs a shared server per `CODEX_HOME` over a
socket, with `thread/loaded/list` returning threads currently in memory. A
TUI attaches to a discovered daemon implicitly, but falls back to an
embedded server when attachment fails, and an embedded server is
unreachable from outside. The interface is marked experimental.

**Ask.** Make shared-daemon hosting reliable or at least discoverable, so
an external read-only client can attach to an interactive session it did
not start. Failing that, document how to tell from outside whether a given
session is reachable, so Deckhand can show an honest `unknown` instead of
guessing.

## 6. What not to ask for

Two things that look like gaps and are not, recorded so they do not get
filed by mistake.

**Session enumeration on Claude Code.** `claude agents --json` already
returns the live sessions. The ask is the missing fields in 4.1, not the
command.

**An observer API on Codex.** It shipped. The hooks system, the thread
status flags, and the app-server notifications together cover observation.
The ask is 5.1 and 5.2, not a new surface.

## 7. Where these get filed

| Ask | Destination |
| --- | --- |
| 4.1, 4.2, 4.4, 4.5, 4.7 | new issues on `anthropics/claude-code` |
| 4.3 | nothing to file; adopt `PermissionRequest` in Deckhand first |
| 4.6 | new issue, once Phase 2 is actually in front of us |
| 3, 5.1 | new issue on `openai/codex` |
| 5.2 | new issue on `openai/codex`, or a question first |

Nothing here has been filed yet. When an ask is filed, add the issue link
to its section. When an ask is answered, say so in the section and open
the work in [TODO.md](../TODO.md) rather than deleting the entry, so the
next person can see what changed and when.

Before filing any of these, re-check it against the current release. 4.3
was stale by a month and nothing in the documentation said so.
