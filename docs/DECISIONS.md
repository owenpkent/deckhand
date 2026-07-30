# Decisions

Status: **accepted**. The log itself is stable; each entry below carries its
own status, per the format note.

Architecture Decision Records, lite format. One entry per decision that would
otherwise get re-litigated in six months. Newest at the bottom. A decision is
changed by adding a superseding entry, not by editing history.

Format: context, decision, consequences. Status is `accepted` unless marked.

---

<a id="adr-001"></a>
## ADR-001: Build a software clone of the Codex Micro

Date: 2026-07-27

**Context.** The Codex Micro (Work Louder and OpenAI) is a macropad that acts
as a status board and command centre for Codex chats: six agent keys with
status LEDs, six command keys, a stick, a dial, push-to-talk. It is a
limited-run physical product that requires functioning hands, and it targets
ChatGPT.

**Decision.** Reimplement the interaction model as an on-screen, pointer-only
surface. Keep the parts that carry the value (per-agent status lamps, always
available actions, the colour language) and drop the parts that are properties
of plastic (pairing, battery, layers-as-firmware).

**Consequences.** Deckhand inherits a proven interaction model instead of
inventing one, and inherits the obligation to say clearly where and why it
diverges. [CONTROL_MAPPING.md](CONTROL_MAPPING.md) is that ledger.

---

<a id="adr-002"></a>
## ADR-002: Tauri v2, Rust daemon, TypeScript surface

Date: 2026-07-27

**Context.** Candidates: Electron plus TypeScript (matches markcopy and
meshview tooling, heavy), PySide6 plus QML (matches alpha-osk, which already
solves the no-focus-steal window on Windows), Tauri v2 (small binaries, Rust
core, webview UI, matches MacroVox). The owner chose Tauri.

**Decision.** Tauri v2. Rust owns the daemon (state machines, approvals,
adapter host); TypeScript owns drawing and pointer input; the webview holds no
authority.

**Consequences.** Small install, one binary, sibling-project precedent in
MacroVox. Two risks accepted: a Rust toolchain is heavier to contribute to
than Python, and the non-focus-stealing always-on-top window is *unproven in
Tauri*. alpha-osk needed raw Win32 `WS_EX_NOACTIVATE | WS_EX_TOPMOST` via
`SetWindowLongW` on top of Qt's flags, reapplied on visibility changes; the
equivalent must be proven in Tauri before Phase 1 code is written. If it
cannot be, this ADR is superseded and the stack question reopens.

---

<a id="adr-003"></a>
## ADR-003: Claude Code first, through an adapter boundary

Date: 2026-07-27

**Context.** The original device drives ChatGPT and Codex. The owner runs
Claude Code daily, often several sessions at once. The ChatGPT desktop app
exposes no status to observe, whereas Claude Code exposes hooks, a documented
permission-decision interface, and an SDK.

**Decision.** Claude Code is the first and reference target. All
runtime-specific knowledge lives behind the adapter contract
([ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md)); the daemon and surface stay
runtime-agnostic. Other runtimes are welcome as future adapters.

**Consequences.** Deckhand is useful to its author on day one, which is the
only reliable engine this project has. The contract stays a guess until a
second adapter exists (see ADR and roadmap Phase 6).

---

<a id="adr-004"></a>
## ADR-004: Attached mode before hosted mode

Date: 2026-07-27

**Context.** Two ways to relate to a session: watch one the user started in a
terminal (attached), or start and own it via the Agent SDK (hosted). Attached
cannot send prompts, because Claude Code has no supported injection interface.
Hosted can do everything but has no terminal UI, making Deckhand responsible
for rendering the session.

**Decision.** Build attached mode first (Phases 1 to 3). Hosted mode is
Phase 4. The mode is per session, not per app, so they can coexist later.

**Consequences.** The first shipped Deckhand improves a workflow that already
exists rather than proposing a new one. The cost is honest but real: in
attached mode, Send stays disabled by default, and some users will feel that
as a gap. The alternative (leading with hosted mode) would delay usefulness
and grow a transcript viewer before the status board is proven.

---

<a id="adr-005"></a>
## ADR-005: Hooks are the status source; transcripts are never load-bearing

Date: 2026-07-27

**Context.** Claude Code state can be observed via documented hooks, or by
tailing transcript JSONL files whose per-line schema is an undocumented
internal that changes between releases.

**Decision.** Hooks (plus the documented status line JSON) are the only
load-bearing observation channels. Transcript reading is permitted only for
optional detail, behind a lenient parser, degrading to nothing.

**Consequences.** Status survives Claude Code upgrades that change internal
formats. Cold start is genuinely harder (hooks only report the future), and
the design accepts grey `unknown` tiles after a daemon restart instead of
guessing.

---

<a id="adr-006"></a>
## ADR-006: The PreToolUse gate is the approval mechanism, and it fails to `ask`

Date: 2026-07-27

**Context.** The `PreToolUse` hook can return
`permissionDecision: allow | deny | ask`, documented. Holding that hook open
while a human decides turns Deckhand into the permission prompt.

**Decision.** Approve and Deny are implemented exactly this way, per session,
off by default. Every non-human exit path (timeout, crash, shutdown,
unreachable daemon) resolves to `ask` or to no output, both of which return
the decision to Claude Code's own UI. No path produces `allow` except a click
or an explicit, attributed rule.

**Consequences.** The approve button rests on a documented interface, not a
scrape. A slow human makes Claude Code prompt twice in effect (Deckhand amber,
then the terminal prompt after `ask`), which is mildly annoying and entirely
safe, and is the accepted trade.

---

<a id="adr-007"></a>
## ADR-007: Loopback HTTP with a token between shim and daemon

Date: 2026-07-27

**Context.** Hook shims are short-lived subprocesses that must deliver JSON
and, for the gate, block for an answer. Candidates: loopback HTTP, named
pipes and Unix sockets, files, a long-lived child.

**Decision.** Loopback HTTP on 127.0.0.1 with a per-install bearer token,
user-only file permissions. Approval *decisions* are only accepted from the
surface IPC, never the HTTP side.

**Consequences.** One transport on all three platforms and trivially testable
with curl. Loopback is not a security boundary against same-user processes;
[SECURITY_MODEL.md](SECURITY_MODEL.md) states the residual risk instead of
pretending pipes would remove it. Revisit if the threat model changes.

---

<a id="adr-008"></a>
## ADR-008: Keep the device's colour language, add `unknown`, never colour-only

Date: 2026-07-27

**Context.** The Codex Micro established white, blue, green, amber, red, off.
Anyone who has seen the device can read Deckhand. But hue-only coding fails
colour-blind users, and a software observer has a state the device never
admitted to: not knowing.

**Decision.** Inherit the six meanings unchanged. Add grey hatched `unknown`,
reported whenever the daemon or adapter cannot tell. Every state carries a
glyph and a label; colour is never the sole channel. Green means unread and
clears on tile selection only.

**Consequences.** Familiarity for free, honesty when observation degrades, and
a testable rule: any code path that would guess a state must emit `unknown`
instead. The cost is a seventh state to design and explain.

---

<a id="adr-009"></a>
## ADR-009: Specification before code

Date: 2026-07-27

**Context.** The two prior repos mark the extremes: alpha-stick is
docs-first (still pre-hardware-validation), alpha-osk is a shipping app whose
design lives partly in a large CLAUDE.md. Deckhand's riskiest parts (the
approval path, the focus behaviour, the honest limits of attached mode) are
exactly the parts cheapest to get right on paper.

**Decision.** Phase 0 ships a full specification and no application code. The
first code (Phase 1) is preceded by two spikes: the Tauri no-focus-steal
window and hook payload validation against a live install.

**Consequences.** The repo is public and reviewable before it is runnable,
and claims stay ahead of nothing: every doc carries its status. The risk of
spec drift once code exists is handled by the change-propagation table in
[WORKFLOW.md](WORKFLOW.md).

---

<a id="adr-010"></a>
## ADR-010: No speech recognition in this repo

Date: 2026-07-27

**Context.** The device has push-to-talk. The author's sibling project
MacroVox already does voice capture and transcription, and audio pipelines
are a maintenance and privacy burden unrelated to status boards.

**Decision.** Deckhand implements the talk *control* (start, stop, indicator)
and delegates capture and recognition to MacroVox or the OS. No audio code,
no audio permissions, in this repo.

**Consequences.** Phase 5 depends on an integration contract with MacroVox
that does not exist yet, and says so. Deckhand never touches the microphone
permission on its own behalf.

---

<a id="adr-011"></a>
## ADR-011: MIT licence, single

Date: 2026-07-27

**Context.** alpha-stick dual-licenses because it has hardware (MIT plus
CERN-OHL-P). Deckhand has no hardware.

**Decision.** MIT for everything.

**Consequences.** One LICENSE file. If the project ever bundles third-party
assets under other terms, they get a NOTICE.md in the alpha-stick pattern.

---

<a id="adr-012"></a>
## ADR-012: The name is Deckhand

Date: 2026-07-27

**Context.** Candidates included switchboard (operator's lamp board),
talkback (studio push-to-talk). The owner picked deckhand.

**Decision.** `deckhand`: a control deck plus the hand that works it, in
service of the person actually steering. Repo `owenpkent/deckhand`, product
name capitalised as Deckhand in prose.

**Consequences.** One-word, unclaimed in the owner's project family, and the
nautical register tolerates the metaphor without a mascot.

---

<a id="adr-013"></a>
## ADR-013: Amber carries a kind, and questions get answer targets

Date: 2026-07-30

**Context.** Measured across 240 local interactive sessions on the owner's
machine, Claude Code asked a multiple-choice question 322 times, in 155 of
those sessions, against 10 to 27 tool denials in the whole corpus. The
surface has a control for approving and one for denying, and none for
answering. Approve therefore lights on a question it cannot answer, which is
the silently wrong button [UI_SPEC.md](UI_SPEC.md) forbids. The evidence is
one user's corpus on one machine, and the docs say so.

**Decision.** `SessionUpdate.detail` carries
`kind: "permission" | "question"`. Approve and Deny are enabled only when
`kind` is `permission`. A question renders each option as its own target,
showing the full option label, never a bare letter or index, at the 44 dip
floor and with the mandated dead gap between adjacent targets. The capability
`answer_question` is declared optional and unproven: no answer channel has
been observed, so it is not `documented`. The protocol type
`PermissionRequest` is renamed `DeckhandPermissionRequest`, because Claude
Code has a hook event of that name with a different shape and the collision
is a trap. [ADR-008](#adr-008) stands unchanged: amber is still amber, with
no new colour, no new state, and no repurposed meaning.

**Consequences.** The most frequent human decision in a real session gets a
control, and the two most prominent controls stop offering to act on
something they cannot act on. The trade accepted: a capability is named
before a channel for it is proven, so answer targets ship disabled with a
stated reason until one is observed. This reopens if an answer channel is
observed, or if a second user's corpus shows questions are not the dominant
amber.

---

<a id="adr-014"></a>
## ADR-014: The default gate is narrow, not `matcher: "*"`

Date: 2026-07-30

**Context.** `PreToolUse` fires on every tool call. On a machine running
`permissions.defaultMode: "auto"`, where a classifier answers most permission
prompts, switching gating on with `matcher: "*"` converts a session that
prompts the human zero times into one amber per tool call, with Deckhand as
the cause of the clicks it exists to remove.
[ACCESSIBILITY.md](ACCESSIBILITY.md) treats those clicks as the scarce
resource, so this is direct harm, not a preference.

**Decision.** Gating ships with an `if` condition scoped to shell execution
and file deletion patterns. `matcher: "*"` stays available and is never the
default. [SECURITY_MODEL.md](SECURITY_MODEL.md) states what amber means under
this gate: a call matching your pattern is waiting, not "Claude Code would
have asked you". `PermissionRequest` has no documented `ask`, so `PreToolUse`
stays the gate and `PermissionRequest` is observation only.
[ADR-006](#adr-006) stands unchanged: every non-human exit path still
resolves to `ask`, never to `allow`.

**Consequences.** Turning gating on costs a bounded number of ambers instead
of one per tool call, so the feature stops fighting the premise of the
product. The trade accepted: a narrow default means Deckhand does not see
calls outside the pattern, so amber is not a complete record of what a
session did, and the docs must say that plainly. The `if` field is
documented, not observed here. This reopens if the shipped pattern proves too
narrow to be useful, or wide enough to hurt once hook overhead is measured.

---

<a id="adr-015"></a>
## ADR-015: The gating hook emits a decision and nothing else

Date: 2026-07-30

**Context.** A `PreToolUse` hook may return more than a decision. Documented
fields include `updatedPermissions`, which writes durable permission rules
into Claude Code, and `updatedInput`, which rewrites the tool input before it
runs. Both are reachable from the same hook Deckhand already holds open.

**Decision.** The gating hook's output is exactly `hookEventName`,
`permissionDecision`, and `permissionDecisionReason`. No other fields.
`updatedPermissions` is forbidden: a durable `allow` created by one click
would be invisible to every later amber and outside the attribution
guarantees of [SECURITY_MODEL.md](SECURITY_MODEL.md) rule 4. `updatedInput`
is forbidden: Deckhand does not edit tool inputs. The allowlist is scoped to
the *gating* hook, so non-gating hooks may still send `additionalContext`. An
"allow always" tile, if it ever ships, is a Deckhand-side rule with
attribution and a one-click disable, never a write into Claude Code's own
permission rules. [ADR-006](#adr-006) stands unchanged.

**Consequences.** Everything Deckhand allows stays attributable to a click or
to an attributed Deckhand rule, and nothing Deckhand does outlives the
session invisibly. The trade accepted: there is no one-click "always allow"
that Claude Code would honour outside Deckhand, so a user who wants that
edits their own settings, deliberately. This reopens if the permission write
interface gains the attribution and revocation that rule 4 requires.

---

<a id="adr-016"></a>
## ADR-016: Liveness by open-operation bracketing, not turn duration

Date: 2026-07-30

**Context.** Measured on the owner's corpus, turn duration runs to p90 660 s,
p95 1,042 s, and p99 2,554 s. A deadline based on turn duration false-greys
healthy sessions at every value that would also catch a dead one.

**Decision.** While any `PreToolUse` lacks a matching `PostToolUse` or
`PostToolUseFailure`, or any `SubagentStart` lacks a `SubagentStop`, the
session stays `THINKING` and the tile shows elapsed-in-operation. There is
one deadline, `T_unknown`, default 900 s; the second stale tier and its badge
are dropped. An open operation suspends the stale clock but not `T_unknown`,
otherwise a killed terminal pins a tile blue forever. The adapter defines
what closes a `PreToolUse` that ends in denial or interrupt. `Task*` events
stay out of the bracketing table: they are teammate-task hooks, not `/tasks`
hooks.

**Consequences.** The liveness question left open in
[ARCHITECTURE.md](ARCHITECTURE.md) has an answer that survives an eleven
minute turn. The trade accepted: bracketing depends on hook events that are
documented but not observed firing here, and a lost close event holds a
session `THINKING` until `T_unknown` demotes it to `UNKNOWN`, which is the
honest failure [ADR-008](#adr-008) asks for rather than a wrong colour. This
reopens if a measured lost-event rate makes 900 s the wrong number.

---

<a id="adr-017"></a>
## ADR-017: `claude agents --json` is a second observation channel

Date: 2026-07-30

**Context.** [ADR-005](#adr-005) made hooks and the documented status line
the only load-bearing observation channels, and accepted grey `UNKNOWN` tiles
after a daemon restart because hooks only report the future. On 2026-07-30,
against Claude Code 2.1.220, `claude agents --json` was run on this machine
and returned live sessions with `pid`, `cwd`, `kind`, `startedAt`,
`sessionId`, `name`, and `status`. It needs no TTY. That is observed, not
inferred.

**Decision.** `claude agents --json` becomes a second documented observation
channel, used to enumerate live sessions at cold start and rebind them by
`session_id`. The capability `list_sessions` moves to
`documented (observed 2.1.220)`. `busy` maps to `THINKING`; everything else,
including a missing status, maps to `UNKNOWN`. Nothing maps to `IDLE` by
guess. `~/.claude/projects/` is demoted to populating the bind picker only.
This supersedes [ADR-005](#adr-005) in part: transcripts stay
non-load-bearing and that clause is untouched, but the set of load-bearing
channels gains this one, which is an enumeration channel rather than an event
stream.

**Consequences.** The all-grey-tiles-after-a-restart failure mode, which is a
daily one, goes away for sessions that are still live. The trade accepted: a
dependency on a CLI surface that can change between releases, and which has
its own off switch, `disableAgentView`. It earns its keep in one place
nothing else covers: `disableAllHooks: true`, `--safe-mode`, and `--bare`
each kill hooks and the status line together while this channel survives, so
a tile in that state can say "hooks are disabled" instead of sitting silently
grey. This reopens if the output shape changes, or if the command stops being
TTY-free.

---

<a id="adr-018"></a>
## ADR-018: Permission mode is a first-class axis, and `auto` is the target

Date: 2026-07-30

**Context.** Claude Code has six permission modes. The owner's machine runs
`permissions.defaultMode: "auto"`, where a classifier answers most permission
prompts before a human sees them. [SECURITY_MODEL.md](SECURITY_MODEL.md)
assumed Deckhand was the only gate on a session, which is false there. A
status board that structurally never lights amber is indistinguishable from a
broken one, and nothing on the surface said which mode a session was in.

**Decision.** `SessionInfo` carries `permissionMode`: the six values plus
`unknown`, because not every payload carries it. The tile shows it as a text
badge, never a colour. `auto` is the design target: the surface is designed
for a machine where the classifier answers most permission prompts and
questions dominate the human's attention, and the docs state plainly what
Deckhand is in each of the other five modes. Disabled Approve and Deny name
the mode as the cause. `ask` returns the decision to a human only in
`default` and `manual`, to the classifier in `auto`, and to a denial in
`dontAsk`; `PreToolUse` still runs first in every mode, and a hook `allow`
still runs in `dontAsk`, so the gate is not dead there. The classifier is
recorded as a second gate in the residual risks. Nothing steps into `dontAsk`
or `bypassPermissions` in one click, and the dial is not repinned to
permission mode. [ADR-006](#adr-006) stands unchanged: Deckhand's own exit
paths still fail to `ask`.

**Consequences.** The board can tell the truth about why it is quiet, and the
one setting that changes what every control means is visible on the tile. The
trade accepted: designing for `auto` means Approve and Deny are correct but
no longer the headline, and the evidence for that choice is one user's corpus
on one machine. Behaviour outside `manual` and `default` stays unverified.
This reopens if a second corpus shows a different mode distribution, or if
per-mode behaviour diverges enough to need a mode-adaptive surface.

---

<a id="adr-019"></a>
## ADR-019: A tile budget, and `COMPLETE` waits for live children

Date: 2026-07-30

**Context.** Measured on the owner's corpus, about 10.5% of turns end with
subagents still running, up to four at once. The tile therefore goes green
while work continues, which falsifies the one promise a status board makes.
Separately, the tile is 96 by 96 dip at 100% scale and has to survive 300%,
and every proposal in flight wants to add something to it.

**Decision.** The tile is fixed at four content slots plus two corner badges,
ranked by value per pixel in [UI_SPEC.md](UI_SPEC.md). Badges are never hit
targets. Slots 3 and 4 collapse in rank order at 200% scale and above.
`COMPLETE` is unreachable while the child ledger is non-empty, and the
bottom-right badge carries the child count. The ledger holds
`kind: "subagent"` entries only, fed by `SubagentStart` and `SubagentStop`;
background Bash tasks emit no hook and are invisible to it, and the docs say
so rather than implying the count is complete. There is no per-child list, no
per-child approval target, and no subagent layer. [ADR-008](#adr-008) stands
unchanged: no new colour, no new state, no repurposed meaning, and green
still means unread and clears on tile selection only.

**Consequences.** Green stops lying, and the tile has a budget to refuse the
next badge with. The trade accepted: a leaked ledger entry, a `SubagentStop`
that never arrives, holds a session blue until `T_unknown`
([ADR-016](#adr-016)) demotes it, which is a visible failure instead of a
silent wrong green. This reopens if four slots prove too tight once the
liveness and permission-mode work lands, or if a documented interface for
enumerating a session's children appears.

---

<a id="adr-020"></a>
## ADR-020: Attached-mode send is unproven, not impossible

Date: 2026-07-30

**Context.** [ADR-004](#adr-004) chose attached mode before hosted mode, and
justified the missing Send with the claim that Claude Code has no supported
injection interface. That premise is too strong. Documented channels do
exist: a Stop hook returning `decision: "block"` with a reason, `SessionStart`
`initialUserMessage`, and `additionalContext`. All of them deliver at a turn
boundary. None delivers into an idle session, which is exactly when a person
wants to type. None has been observed here.

**Decision.** This refines the premise of [ADR-004](#adr-004), not its
decision. Attached mode still comes before hosted mode, and `send_prompt`
stays `false` in attached mode. The flat "no supported way" claim is deleted
and replaced by what exists, what shape it has, and the accurate narrower
claim: no documented channel delivers a prompt into an idle session at an
arbitrary time. A Phase 1 spike observes Stop-hook block behaviour on a live
install, including whether the turn stays the same session and what ceiling
exists on holding it open. No capability is promoted on the strength of
documentation alone.

**Consequences.** [ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md) rule 7 holds:
confidence is declared honestly, and a channel that cannot deliver when the
user wants it does not earn `documented`. Any control that needs a write
channel, Continue in particular, ships disabled with an honest reason, or as
`synthetic` if the user opts in. The trade accepted: the docs now describe a
channel Deckhand deliberately does not use, which invites the question every
time someone reads them. That is cheaper than leaving a false claim in place.
This reopens when the spike reports.

---

<a id="adr-021"></a>
## ADR-021: No tooltip-only reveals

Date: 2026-07-30

**Context.** Five load-bearing reveals in [UI_SPEC.md](UI_SPEC.md) and
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md) were tooltips.
[ACCESSIBILITY.md](ACCESSIBILITY.md) forbids hover as a required interaction,
and the surface never takes focus, so a dwell user or an eye-tracker user has
no route to a tooltip at all. This was an existing violation, not a new risk.

**Decision.** Every load-bearing tooltip becomes a click-to-reveal into the
detail panel. Clicking a disabled control is never a no-op: it reveals why
that control is disabled. If the panel is collapsed, the same click expands
it.

**Consequences.** The reason a control is off is reachable with the only
input this product assumes anyone has. The trade accepted: one extra click
for a mouse user who could have hovered, and more content for the detail
panel and the tile budget ([ADR-019](#adr-019)) to carry. Being an
accessibility requirement, this does not reopen on cost grounds; it reopens
only if [ACCESSIBILITY.md](ACCESSIBILITY.md) itself changes.
