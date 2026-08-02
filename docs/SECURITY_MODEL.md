# Security model

Status: **proposed**. Written before the code so the code has something to be
held against.

## Why this file exists

Most on-screen control surfaces are harmless: worst case, they click the wrong
button. Deckhand is different in exactly one place. When permission gating is
enabled, **Deckhand's daemon can answer "allow" to a Claude Code tool call**,
which can mean running a shell command, editing a file, or touching the
network. That single capability is what this document is about. Everything else
is ordinary local-app hygiene.

## Assets

| Asset | Why it matters |
| --- | --- |
| The permission-decision channel | Can authorise tool execution in your sessions |
| Session metadata on the daemon | Project paths, tool names, tool inputs: reveals what you are working on |
| Tool inputs shown for approval | Can contain source code, secrets in command lines, file contents |
| The optional audit log | A durable copy of the above |
| Claude Code settings files | Deckhand writes hook entries into them |

## Trust boundaries

```
 you (pointer) ──► surface ──► daemon ◄── loopback ◄── hook shims ◄── claude
                                 ▲
                    other local processes (untrusted)
```

1. **The pointer to the surface.** Anything that can move your pointer can
   press Approve. That includes you on a bad day; see misclicks below.
2. **The loopback endpoint.** Every process running as any local user can open
   a socket to 127.0.0.1. Loopback is a convenience, not a boundary.
3. **The settings files.** Whatever else edits `settings.json` can add hooks of
   its own; Deckhand must not make that easier or harder to notice.

Out of scope: Claude Code itself, the model's behaviour, Anthropic's services,
and an attacker who already runs code as you with full user rights (such an
attacker does not need Deckhand; they can edit `settings.json` directly).
In scope: Deckhand must not *lower* the bar, and must not turn a low-privilege
foothold into an approval press.

## The rules

### 1. Fail closed, in the correct direction

"Closed" for a permission gate does not mean deny, it means **return the
decision to the user**. Every failure path answers `ask`:

| Failure | Answer |
| --- | --- |
| No human decision before Deckhand's deadline | `ask` |
| Surface not running or not visible | `ask` |
| Daemon shutting down with approvals pending | `ask` for each |
| Shim cannot reach the daemon | No output at all; Claude Code behaves as if Deckhand were not installed |

`allow` is only ever produced by an explicit click or an explicit rule. There
is no code path where a timeout, a crash, a default, or a missing config value
produces `allow`.

The last row is worth stating exactly, because "prompts normally" would be an
overstatement. With no hook output the call falls through to whatever Claude
Code would have done unhooked, which is a prompt in some modes, the classifier
in `auto`, and execution in `bypassPermissions`. That is the same outcome as
never installing Deckhand, which is the correct behaviour for a dead
companion: a broken status board must not brick every session on the machine.
It is not a Deckhand `allow`, and no Deckhand code path emits one.

**What `ask` reaches depends on the session's permission mode.** In `manual`
it returns the decision to a human, which is what the rest of this
rule assumes. In `auto` it returns the decision to Claude Code's own
classifier, which can answer without a person ever seeing the call. In
`dontAsk` it becomes a denial. Deckhand cannot change any of that, so it says
it plainly: `ask` is the safest answer Deckhand can give in every mode, and in
some modes it is not the same thing as "a human decides". The mode travels with
the session and is shown on the tile as text, never as a colour, so a
fail-closed exit is never silently reinterpreted.

There are six modes, and this rule accounts for all six. `claude --help` on
2.1.220 lists exactly `acceptEdits`, `auto`, `bypassPermissions`, `manual`,
`dontAsk`, and `plan` as the choices for `--permission-mode`, which is an
observation, not a reading. `manual`, `auto`, and `dontAsk` are the three
that carry a claim, above. The remaining three in
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types), `acceptEdits`, `plan`, and
`bypassPermissions`, get no claim here beyond the one that holds in all of
them: Deckhand still emits `ask` and still never emits `allow` on a failure
path. Where an `ask` lands in each of the three has not been observed and is
not asserted, so a tile in one of them shows the mode as text and Approve and
Deny name it as the reason they are off. An unverified guess about a
permission mode is exactly the thing this document exists to refuse.

Mode does not switch the gate off. `PreToolUse` still runs first in every mode,
and a hook `allow` still runs the call in `dontAsk`, so the gate is live even
where Claude Code itself would not have prompted. Mode changes what a
fall-through means, not whether Deckhand is asked. (Documented for Claude Code
2.1.220; unverified here, since no mode other than `manual` has been seen to
run a hook on this machine.)

Claude Code 2.1.220 documents a fourth decision value, `defer`. Deckhand never
uses it on a failure path. `defer` falls through to the normal permission flow,
which is the classifier in `auto` and execution in `bypassPermissions`, so it
is not fail-closed in the modes where that matters most. `ask` stays the
fail-closed exit ([ADR-006](DECISIONS.md#adr-006), unchanged).

### 2. Authenticate the loopback

- The daemon binds 127.0.0.1 only. Never `0.0.0.0`, not even in debug builds.
- Every request carries a bearer token generated per install, stored in the
  user's config directory with user-only permissions, and rotated on demand.
- The shim reads the token from that file, so possession of the token proves
  the caller can already read the user's files. This is honest about what
  loopback auth can prove: it stops other users on the machine and unprivileged
  sandboxed processes, and it does not stop a process running as you. That
  residual risk is accepted and documented rather than hidden.
- Decisions themselves (approve, deny, settings changes) are accepted **only
  from the surface's own IPC channel**, never from the HTTP endpoint. The HTTP
  endpoint reports and asks; it cannot answer.

### 3. Protect the click

An approval press must be a considered act even though it is one click.

- Approve and Deny enable only when the selected tile is amber **and the
  pending item is a permission request** (`kind: permission`), and only for the
  request being displayed. There is no "approve whatever is pending". Amber
  raised by a question enables that question's answer targets instead; Approve
  is never live over something it cannot approve.
- A disabled Approve or Deny is never a silent no-op. Clicking it reveals the
  cause in the detail panel: nothing pending, the wrong kind of amber, or a
  permission mode in which the decision would not have reached you anyway.
- **No approval control may appear under the pointer within 500 ms of another
  interaction at that spot.** This kills the misclick where a tile press lands
  on a button that materialised beneath it.
- The approval card always shows tool name and tool input before the buttons
  are live. Unreadable input (too long, binary) renders truncated with an
  expand affordance, but the buttons stay live: forcing a scroll-to-approve
  would punish exactly the users this tool is for.
- A configurable per-tool "confirm twice" list, defaulting to shell execution
  and file deletion patterns.
- Approvals are per request. Batch approval is a rule, and rules are explicit.

### 4. Keep rules legible

Auto-approval rules (unscheduled: they reach a phase only through
`ROADMAP.md`, and ship off by default) are the riskiest convenience in
the design, because they turn a security decision into configuration.

- Rules can only *narrow*: a rule matches a tool name pattern and optionally a
  session, and may answer `allow`, `deny`, or `ask`. There is no "allow all".
- Every rule-made decision is visibly attributed on the tile and in the detail
  panel: what answered, which rule, and a one-click way to disable it.
- Rule changes take effect for *future* requests only, never a pending one.
- If the audit log is on, rule decisions are logged identically to human ones.

### 5. Respect the data

- Nothing leaves the machine. No telemetry, no crash reporting, no update
  pings in Phase 1 to 3. If any of that is ever proposed, it is opt-in and
  gets its own decision record.
- Tool inputs are held in memory for display and dropped with the request.
  They appear on disk only if the audit log is enabled, and the audit log
  stores the tool name and a hash of the input by default, full input only if
  explicitly configured.
- Deckhand reads transcripts where Claude Code already keeps them and copies
  nothing out of them.
- Logs default to metadata, not content.

### 6. Touch other tools' config like a guest

- The hook block is shown in full and applied only on explicit confirmation.
- Only Deckhand's own entries are ever removed.
- A settings health check warns when Deckhand's entries are present but the
  daemon is not running, and offers removal, because a dangling gate hook is
  latency with no benefit.
- The same health check detects duplicate gating entries, whether left by a
  second Deckhand install or by an uninstall that did not clean up. Two gating
  entries mean two decisions for one call, and how Claude Code resolves that is
  undocumented.
- Deckhand writes to the user-level `~/.claude/settings.json` only, never to a
  project's `.claude/settings.json`, which is usually committed and would
  publish a machine-local shim path to everyone who clones the repository.

### 7. The gate is narrow by default

`PreToolUse` fires on every tool call, so the scope of the gate is a safety
setting and an accessibility setting at the same time. A gate installed with
`matcher: "*"` turns a session that was prompting for nothing into one amber
per tool call, which makes Deckhand the cause of the clicks it exists to
remove. On a machine running `auto`, where the classifier already answers most
permission prompts, that is the whole difference between saving motion and
manufacturing it.

- Gating ships scoped by an `if` condition covering shell execution and file
  deletion patterns, never `matcher: "*"`. (The `if` field is documented for
  Claude Code 2.1.220 and unverified here.)
- Widening the scope is a deliberate act with its cost stated at the point of
  the change, and it is reversible in one click.
- `PermissionRequest` is an observation event, not a second gate. It has no
  documented way to answer `ask`, so `PreToolUse` remains the only place
  Deckhand can decide anything.

**What amber means under a gate.** Amber means "a tool call matching your gate
pattern is waiting for you". It does not mean "Claude Code would have asked you
about this". The two coincide only in `manual` mode with a
pattern no wider than what Claude Code would have prompted for anyway. Narrow
the pattern and amber stays rare and worth reading; widen it and amber becomes
the cost of running Deckhand at all. Nothing in the surface may present a
gate-generated amber as though Claude Code raised it.

### 8. The gating hook writes nothing but a decision

The gating hook's output is exactly three fields, and no others:

| Field | Value |
| --- | --- |
| `hookEventName` | `PreToolUse` |
| `permissionDecision` | `allow`, `deny`, or `ask` |
| `permissionDecisionReason` | Short text naming what answered: a click, or a rule |

Two fields are forbidden by name, because both are ways for one click to reach
further than the request in front of it:

- **`updatedPermissions` is forbidden.** It writes a durable permission rule
  into Claude Code. A single approval would then answer every later matching
  call, invisibly to every later amber, and entirely outside the attribution
  and one-click-disable guarantees of rule 4. If an "allow always" control ever
  ships, it is a Deckhand-side rule under rule 4, never a write into Claude
  Code's own permission rules.
- **`updatedInput` is forbidden.** Deckhand approves or denies the call as
  presented. It never edits a tool input, because the input shown on the
  approval card has to be the input that runs.

This allowlist scopes to the *gating* hook only. Deckhand's observation hooks
answer no permission decision, and may carry whatever their own event supports,
`additionalContext` among it.

## Residual risks, stated plainly

1. **A process running as you can press Approve** by driving the surface's IPC
   or synthesising pointer input. Deckhand does not defend against same-user
   code execution; nothing user-mode can.
2. **The always-on-top surface can overlap other apps' UI.** It never accepts
   focus and never covers its own approval card, but it could sit over another
   app's dialog. Mitigation is placement control and a collapse gesture, not a
   claim that overlay problems are solved.
3. **Synthetic input fallbacks** (attached-mode send, focus raising) type into
   whatever window matches a heuristic. That is why they are off by default,
   marked `synthetic`, and never combined with approval authority. The
   heuristic is weaker on a `vscode-extension` host, where a pid identifies no
   single window and the title is all there is, so send has no synthetic route
   there at all and Reveal raises a window without selecting a tab. The
   approval path is unaffected by any of this: it runs over hooks and is
   host-independent, which was observed rather than assumed
   ([DECISIONS.md](DECISIONS.md#adr-023)).
4. **A malicious prompt could time its tool call** so amber appears just as you
   click elsewhere. The 500 ms materialisation rule and per-request display are
   the mitigations; they reduce, not eliminate.
5. **Deckhand is not the only gate on a session.** In `auto` mode Claude Code's
   own classifier decides most permission prompts, and that mode is the default
   on the author's machine. Calls Deckhand never sees may already have been
   allowed or denied, and the `PermissionDenied` event usually carries the fixed
   reason "Blocked by classifier", which says almost nothing about why. Deckhand
   reports what reaches it and must never present that as the whole record of
   what a session was permitted to do.
6. **Two decision sources can disagree.** Two Deckhand installs, or one install
   plus an orphaned shim entry, put two gating entries in the settings file, and
   how Claude Code resolves conflicting decisions across them is undocumented
   and unverified. Until it has been observed, the mitigation is prevention
   rather than reconciliation: the health check in rule 6 detects duplicate
   entries and offers to remove the ones that are not this install's.

## Reporting

Same channel as everything else: see [SECURITY.md](../SECURITY.md). Private
disclosure via GitHub Security Advisories, no public issues for
vulnerabilities.
