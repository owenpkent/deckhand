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
| Shim cannot reach the daemon | No output at all; Claude Code prompts normally |

`allow` is only ever produced by an explicit click or an explicit rule. There
is no code path where a timeout, a crash, a default, or a missing config value
produces `allow`.

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

- Approve and Deny enable only when the selected tile is amber, and only for
  the request being displayed. There is no "approve whatever is pending".
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

Auto-approval rules (Phase 3+, off by default) are the riskiest convenience in
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
   marked `synthetic`, and never combined with approval authority.
4. **A malicious prompt could time its tool call** so amber appears just as you
   click elsewhere. The 500 ms materialisation rule and per-request display are
   the mitigations; they reduce, not eliminate.

## Reporting

Same channel as everything else: see [SECURITY.md](../SECURITY.md). Private
disclosure via GitHub Security Advisories, no public issues for
vulnerabilities.
