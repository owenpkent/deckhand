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

---

<a id="adr-022"></a>
## ADR-022: `default` is not a permission mode; `manual` is

Date: 2026-07-30

**Context.** [ADR-018](#adr-018) named `default` as one of Claude Code's six
permission modes and left `manual` out of the enum in
[ADAPTER_PROTOCOL.md](ADAPTER_PROTOCOL.md#types), which then carried seven
values instead of six. Running `claude --help` on 2.1.220 on this machine
lists the `--permission-mode` choices as exactly `acceptEdits`, `auto`,
`bypassPermissions`, `manual`, `dontAsk`, and `plan`. `default` is not among
them.

**Decision.** The membership is corrected everywhere the modes are named: the
protocol enum is the six above plus Deckhand's own `unknown`, and the phrase
pairing "`default` and `manual`" as the modes where an `ask` reaches a human
becomes `manual` alone. ADR-018's count of six stands and its decision stands;
only its naming was wrong, so this corrects ADR-018 rather than superseding
it. ADR-018 is not edited, per this file's own rule that a decision is changed
by adding an entry, not by rewriting history. Whether the settings key
`permissions.defaultMode` additionally accepts a value spelled `default` is
**unverified**: the owner's own setting is `auto`, the CLI flag rejects the
spelling, and neither document nor experiment here settles the key. Nothing in
the spec assumes either answer. [ADR-006](#adr-006) and [ADR-008](#adr-008)
stand unchanged.

**Consequences.** One fact moves from `documented` to `observed`, and the
verification stamp in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) now marks four things rather
than three. It moved because a command was run, which is the standard the
Phase 1 hook payload spike is held to and the reason that spike stays open:
reading a name in documentation is not seeing it fire. The trade accepted: an
enum written from documentation was wrong in a way nobody would have caught
without running the binary, which is an argument for running it earlier. This
reopens if a later Claude Code release changes the accepted set, or if the
settings key is tested and accepts something the flag does not.

---

<a id="adr-023"></a>
## ADR-023: The host is a third axis, and capabilities belong to a session

Date: 2026-08-02

**Context.** [ADR-004](#adr-004) and the spec since have described two ways
of relating to a session, attached and hosted, and treated attached as a
synonym for "running in your own terminal". The owner runs Claude Code
through the VS Code extension as well, which that model has no place for.
A spike on 2026-08-02, against Claude Code 2.1.220 on Windows 11, observed
the following.

An extension-hosted session is a real `claude.exe`, launched from
`.vscode/extensions/anthropic.claude-code-2.1.220-win32-x64/resources/native-binary/`
with `--output-format stream-json --verbose --input-format stream-json`,
parented to a `Code.exe` utility process, owning no window of its own. It
appears in `claude agents --json` tagged `kind: "interactive"`, which is
what a terminal session is tagged, so `kind` does not discriminate hosts.
No `status` key was present on any row of that output, which corrects the
key list in the stamp in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

Hooks are unaffected by the host. This repository's own `PreToolUse` gate
fired from inside the extension, with `tool_name` and `tool_input`
populated, and its `permissionDecision: "deny"` was honoured. That is the
first time a hook has been seen to fire and decide anything here, and it
was seen in the host the spec did not model.

The extension also runs an MCP server over WebSocket, one per VS Code
window, advertised at `~/.claude/ide/<port>.lock` carrying `pid`,
`workspaceFolders`, `ideName`, `transport`, and an `authToken`, and reached
with the header `x-claude-code-ide-authorization`. It identifies as
`Claude Code VSCode MCP 2.1.220` and serves twelve tools: `openFile`,
`openDiff`, `getDiagnostics`, `getOpenEditors`, `getWorkspaceFolders`,
`getCurrentSelection`, `getLatestSelection`, `checkDocumentDirty`,
`saveDocument`, `close_tab`, `closeAllDiffTabs`, and `executeCode`. None of
them sends a prompt, interrupts a turn, or reports session state. The
channel exists so the CLI can drive the editor, not so anything can drive
Claude, and it points the wrong way for Deckhand's purposes.

Two further facts decide Reveal. Every VS Code window shares one main
process, so a `pid` cannot tell two windows apart: three live windows and
three live lockfiles all reported the same `pid`. And `openFile` with
`makeFrontmost: true` changed the active tab in the targeted window, which
its window title confirmed, but left the OS foreground window untouched.
The tool does tab focus, not window raise, exactly as its own schema says.

**Decision.** The host becomes a third axis, separate from the mode.
`SessionInfo` gains a `host` field with the values `pty`,
`vscode-extension`, and `sdk`, derived from the process argv and parent
rather than from `kind`, which cannot carry it. `mode` keeps its existing
two values and its existing meaning, which is who started the session.

Capabilities move from the adapter to the session. One adapter now spans
hosts whose capability sets genuinely differ, so a single
`capabilities` record on the `Adapter` can no longer be true: on the
Claude Code adapter `focus_session` is `synthetic` on a `pty` host and
`internal` on a `vscode-extension` host at the same moment. The adapter
declares the capabilities it can ever offer, and each `SessionInfo`
carries the set that actually applies to it. Where they disagree, the
session wins, and the surface reads the session.

Reveal on a `vscode-extension` host raises the window natively, by
enumerating top-level windows and matching the workspace name in the
title, because no `pid` can do it. Tab-level focus is not attempted: the
only thing that could do it is the `claude-vscode.focus` command, which
is invokable only from inside the extension host, and Deckhand will not
ship a companion VS Code extension to reach it. `openFile` is explicitly
rejected as a Reveal primitive: it would navigate the window away from the
Claude tab, hiding the thing the user asked to see.

`send_prompt` and `interrupt` stay `false` on a `vscode-extension` host,
and the reason is stronger than the one that keeps them false on a `pty`
host. On a `pty` host they are unproven, per [ADR-020](#adr-020). Here
they are absent: the full tool and command surface was enumerated and
neither exists, and the process's stdin belongs to the extension. The
opt-in synthetic keystroke fallback is unavailable on this host, having no
window to type into.

The `~/.claude/ide/` lockfiles and the MCP server they advertise are
`internal`, evidence `observed`. Under the rule in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) that `internal` rows may
not be load-bearing, nothing in the spec is allowed to depend on them.
They are recorded because they are the map of what this host does and does
not offer, not because anything is being built on them.

**Consequences.** The spec now describes the host the owner actually uses
for a good part of the day, and the answer for that host is better than
expected on the part that matters and worse on the part that does not.
Status, approve, deny, the mode badge, and the bind picker are unchanged,
because hooks come out of `claude.exe` and not out of whatever is holding
its pipes. Reveal degrades from "raise the right thing" to "raise the
right window", which in the owner's observed setup, one workspace per
window, is the same thing.

[ADR-006](#adr-006) stands and is strengthened: the approval path is
host-independent, and that is now observed rather than assumed.
[ADR-008](#adr-008) is untouched, no state or colour moves.
[ADR-020](#adr-020) stands for the `pty` host and is narrowed, not
superseded: its "unproven, not impossible" verdict was about the terminal,
and this entry adds a host where the stronger word is warranted.

The trade accepted: a second discriminator on every session, and a
capability model that is now two-level and therefore easier to get wrong,
bought in exchange for a spec that stops being silent about a third of how
the product is used. This reopens if Claude Code exposes a documented way
to send into or interrupt a running session from outside it, on any host,
or if a release adds a window-raise tool to the IDE server.

<a id="adr-024"></a>
## ADR-024: `claude agents --json` recovers bindings, not state

Date: 2026-08-02

**Context.** [ADR-017](#adr-017) made `claude agents --json` a second
observation channel on the strength of a run that appeared to return a
`status` key alongside `pid`, `cwd`, `kind`, `startedAt`, `sessionId`, and
`name`. Its consequences claimed that the all-grey-tiles-after-a-restart
failure mode "goes away for sessions that are still live", and it closed by
naming its own reopen condition: "This reopens if the output shape changes."

The re-run on 2026-08-02, recorded in [ADR-023](#adr-023), found no `status`
key on any row. A third run while reviewing that change confirmed it: six
rows, six keys each, no `status` anywhere. Whether the key was misread on
2026-07-30 or removed since does not matter to the outcome. The reopen
condition is met either way.

**Decision.** ADR-017 is narrowed, not superseded. `claude agents --json`
stays a load-bearing observation channel and `list_sessions` keeps its
`documented (observed 2.1.220)` confidence, because enumeration is the part
that was observed and re-observed. What it no longer carries is a state
claim.

At cold start the channel recovers the binding and the label. It does not
recover the state. The `busy` to `THINKING` mapping in ADR-017 is kept in the
spec as a conditional, since it costs nothing if the key returns, but on
2.1.220 it never fires: every enumerated session takes the missing-status
branch and lands in `UNKNOWN`. Nothing may be written that depends on a
status arriving from this channel.

**Consequences.** The honest version of the win is smaller than ADR-017's and
still worth having. Before this channel, a daemon restart left tiles grey,
unbound, and unnamed, and the only route back was to wait for each session to
emit an event or to rebind six tiles by hand. After it, the tiles are still
grey, but they are the right tiles, bound to the right sessions, under the
right names, and the first event on any of them colours it correctly. The
fail-safe in ADR-017 is what makes this survivable: because a missing status
already mapped to `UNKNOWN` rather than to `IDLE`, the wrong reading produced
no wrong colour, only an overstated claim.

`disableAgentView` and the hooks-disabled case from ADR-017 are unaffected: a
tile can still say "hooks are disabled" on the strength of an enumeration
that answers while hooks do not.

This entry is a correction to a verification stamp, not a design change, so
no control, colour, state, or capability moves. It reopens if a release adds
a status field to the enumeration, at which point the conditional in step 2
starts firing on its own.

<a id="adr-025"></a>
## ADR-025: Tauri clears the no-focus-steal bar on Windows

Date: 2026-08-02

**Context.** [ADR-002](#adr-002) chose Tauri with its biggest risk held
open, and [ADR-009](#adr-009) gated Phase 1 on proving it: an always-on-top
window on Windows 11 that takes mouse clicks without ever taking the
foreground. alpha-osk proves the Win32 recipe, `WS_EX_NOACTIVATE` plus
`WS_EX_TOPMOST`, in PySide6. It does not prove a Tauri window can reach the
same behaviour, because Tauri's content area is a WebView2 child window
with focus habits of its own, and a surface that grabbed the keyboard on
every click would invert the product for a mouse-only user.

**Decision.** The spike at `spikes/tauri-focus/` answers it: the recipe
holds in Tauri v2. Phase 1 builds the window this way, and ADR-002 stands
with its riskiest unknown closed.

What the spike observed, on Windows 11 against Tauri 2 and the installed
WebView2 runtime, recorded in the app's own `spike-log.jsonl`:

- Tauri's window options get halfway there. `alwaysOnTop: true` produced
  extended style `0x40118`: `WS_EX_TOPMOST` set, `WS_EX_NOACTIVATE`
  absent. `focus: false` kept the window from activating at creation.
  No Tauri option supplies the missing bit.
- One `SetWindowLongPtrW` call in the setup hook adds it, taking the
  extended style to `0x8040118`, with one `SetWindowPos` carrying
  `SWP_NOACTIVATE` to re-assert topmost placement. Nothing fought the
  change back.
- A synthetic click on a button inside the webview ran the DOM click
  handler and reached a Tauri command, while `GetForegroundWindow` before
  and after the click returned the same other window and never the spike.
  The click landed without activation.
- With Chrome holding the foreground and the machine in live use,
  starting the spike and clicking at it moved nothing: Chrome kept the
  foreground throughout.

**Consequences.** Half the ADR-009 gate is closed; the other half, hook
payload validation, advanced the same day but stays open. The Phase 1
window inherits the spike's mechanism: config for `alwaysOnTop` and
`focus: false`, plus a one-time Win32 extended-style pass at setup, behind
a `cfg(windows)` boundary that an eventual macOS or Linux port replaces
rather than shares.

Hedges, so this entry does not claim more than two runs of one window: the
click-received and unrelated-app-foreground observations come from
separate runs, because the machine was in live use and synthetic input was
stopped rather than risk clicking into the owner's session. The webview
document fires DOM focus events on click even though the OS foreground
never moves; keyboard routing follows the foreground by definition, but
Phase 1 should re-check that dial and scroll interactions do not change
the answer. Dragging, DPI changes, multiple monitors, and release builds
(`windows_subsystem = "windows"`) are untested. This reopens if a Tauri or
WebView2 release changes activation behaviour, or if the Phase 1 window
observably takes focus in daily use.
