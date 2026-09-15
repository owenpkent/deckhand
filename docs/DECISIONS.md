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

<a id="adr-026"></a>
## ADR-026: First live validation, and what reality corrected

Date: 2026-08-02

**Context.** The Phase 1 skeleton shipped proven only against synthetic
events. The same day, the payload capture hook and the machine-local shim
wiring put real traffic through it: this repository's own sessions, a
headless `claude -p` run, and a spawned subagent, all against Claude Code
2.1.220. Nine of the twelve documented hook event names have now been
seen firing here; `Notification`, `StopFailure`, and `PermissionDenied`
have not.

**Decision.** Reality won four arguments, and the code and the spec now
follow it:

- `PostToolUseFailure` carries `error` as a plain string of the tool's
  own output, plus an `is_interrupt` boolean. There is no `error_type`
  field; the status-inference table had specified one from
  documentation. The adapter derives the error detail from the observed
  fields, truncated to panel size, and an `is_interrupt` failure closes
  every operation open on the session, which is the first observed
  mechanism for the interrupt rule in
  [ARCHITECTURE.md](ARCHITECTURE.md#liveness-by-open-operation).
- Subagent events carry `agent_id` and `agent_type`, and `SubagentStart`
  opens a real operation. The child ledger keys on `agent_id` instead of
  counting, which is what makes duplicate delivery a no-op (adapter rule
  4) and stops a stray `SubagentStop` from closing an unrelated tool
  bracket, a bug the counter version had.
- `claude agents --json` emits valid JSON and exits 255. The daemon
  treats parseable output as the success signal and ignores the exit
  code, which its first implementation did not, so cold start silently
  returned nothing against the real binary.
- `permission_mode: "default"` is a live payload value: the headless
  session reported it on `UserPromptSubmit` and `Stop`, even though the
  CLI flag rejects that spelling. The protocol's mode list gains it as
  an observed payload value. What `default` does to an `ask` stays
  unobserved, like every other mode's behaviour.

Also observed, strengthening rather than correcting: hooks fire in print
mode; hook registration changes take effect mid-session without a
restart, seen when events registered mid-session began firing from the
session that registered them; `SessionStart` arrived with
`source: "startup"` and `SessionEnd` with `reason: "other"`, a value the
documented list did not name, which the unrecognised-reason arm already
handled by design.

**Consequences.** The per-claim stamps in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) move accordingly, and
the Phase 0 validation item narrows to the three unobserved events. Red
keeps its narrow promise: `StopFailure` remains a string in a binary,
not an observation. The board itself ran through all of this live, with
the owner's own session as the first tile. This entry reopens per event
as the remaining three are seen, and per field if a release changes any
observed shape.

<a id="adr-027"></a>
## ADR-027: A tile click selects and raises

Date: 2026-09-13

**Context.** The MVP is two things: see the state of every Claude Code
session on this machine, and get to the window holding one of them in a
single click. The control mapping had split that second half in two:
clicking a tile selected the session and left window focus alone, and a
separate Reveal target on the tile, plus the Reveal key and the panel
action, raised the host window. Divergence 1 justified the split by the
surface's need never to steal focus, which is a different thing: that
rule is about Deckhand's own window, settled by ADR-025, and it is not
touched by moving the foreground to a session's window on request.

Three ways to fold the raise into the click were weighed. Keep the
split, with the tile carrying two 44 px targets. Raise on the second
click of an already selected tile, which keeps one target per tile but
makes the raise a two-click action and depends on the selected state
being obvious. Raise on every click, which gives up looking at a session
without going to it.

**Decision.** Clicking a bound tile selects the session and raises its
host window in the same click. There is no separate Reveal target on
the tile and no double-click accelerator, since there is nothing left
for one to accelerate. Deckhand's own window still never activates
(ADR-025): the one focus change the surface makes is to the session's
window, never to itself. The Reveal command key and the panel action
stay as a repeat of the raise for the selected session, which the same
click on the selected tile also does; the key's slot is therefore
redundant and is open for retabling in Phase 3.

The window match itself is unchanged by this entry and remains
unproven live: the pid the enumeration reports belongs to `claude.exe`,
which owns no top-level window on any host observed so far, so the pid
score never fires and the title heuristic carries the whole load. With
every click now attempting the raise, `reveal.log` fills from normal
use, which is how the match gets fixed. That fix, capturing the session
pid and host from the hook process's own environment, is separate work
and gets its own entry.

**Consequences.** The cost is the one the owner accepted knowingly: a
session cannot be inspected on the board without its window coming
forward, and in Phase 2 approving a prompt on one tile brings that
session's window up over whatever was being read. The gain is that the
MVP's second half is one click on a full-size target. Approve and deny
are unaffected: they act on the selected session over hooks, and the
selection rule (green clears on select, divergence 6 in the control
mapping) is the same as before. The control mapping, the accessibility
table, the UI spec, the architecture's focus rule, and the security
model's residual risk on synthetic input are updated in the same change.

---

<a id="adr-028"></a>
## ADR-028: The surface narrows to a session list

Date: 2026-09-13

**Context.** Two things landed at once and pulled in opposite directions.
The change just before this one built out the rest of the physical
control set against the observation-only surface: six command keys,
a stick, a dial, talk and send placeholders, a detail panel, a bind
picker, corner badges. None of it has write authority yet, all of it
is Phase 2 or later, and Phase 1's actual goal is narrower than any of
it: prove that session status can be watched reliably. The second thing
is a use-it-and-find-out result. Hooks were only ever wired into this
repo's own `.claude/settings.local.json`, and the `claude agents --json`
enumeration channel from [ADR-017](#adr-017) and
[ADR-024](#adr-024) has, since it was added, only ever rebound one of
six fixed slots that a human had already bound by hand; it has never
created a binding on its own. The practical result is that a session in
any other repo has never once shown up on the board, which is the
opposite of what a status board across several concurrent sessions is
for.

Both point at the same fix: cut the surface down to the part that is
actually load-bearing for observation, and make binding automatic
instead of a fixed, manually populated set of six.

**Decision.** The surface becomes an ordered, vertical list, one row
per session: status colour, glyph, session name, and the state word,
[ADR-008](#adr-008) unchanged. Clicking a row selects the session,
clears its unread green, and raises its host window in the same click,
exactly as [ADR-027](#adr-027) already decided; only the container
changes from a tile to a row. The header keeps exactly two controls:
Move (the existing click-to-place alternative to dragging the window)
and Quit. Everything else built out in the change just before this one
comes back off the surface: the six command keys (approve, deny,
answer, interrupt, continue, reveal), the stick, the dial, talk and
send, the detail panel, the bind picker, the layer strip, and the two
corner badges. Approve and deny stay exactly where [ROADMAP.md](../ROADMAP.md)
already put them, Phase 2 work with no write authority yet; what changes
is that they are no longer planned to land on this surface as
designed here. A control returning to the surface, approve and deny
included, needs its own ADR, because this entry is what removed the
place they were going to go.

Binding stops being a fixed set of six slots a human fills. It is now
an unbounded, ordered list. A session is auto-bound the first time it
is seen, from any hook event or from a `claude agents` enumeration,
and the daemon now reruns that enumeration every 15 seconds on its own
timer, outside the registry lock, so a slow or hanging enumeration
call cannot stall hook ingestion. A session leaves the list when it
ends, or when a successful enumeration no longer lists it and it has
gone 60 seconds with no hook event either; a failed enumeration call
prunes nothing; missing information is never grounds for removing a
row. [ADR-024](#adr-024) still stands exactly as written: enumeration
recovers a binding and a label, never a state, so a session bound only
by enumeration renders `unknown` until a hook for it actually arrives.
A legacy `bindings.json` from the six-slot design loads by dropping
its null slots and keeping the rest, in order.

The window becomes a vertical list about 360 logical pixels wide, its
height following the row count with each row at least 48 px, and the
whole window clamped into the monitor's work area. A saved window
position is validated against the monitors actually connected at
startup rather than trusted blindly, which fixes one of five findings
from the PR 10 review: saved coordinates surviving a monitor's removal
used to be able to place the window off every current screen. The
raise (Reveal, in the old naming) now excludes Deckhand's own window
from its candidates, which fixes a second finding from the same
review: nothing stops a title match from finding Deckhand itself. The
review's other three findings, panel expansion crowding a screen edge,
the panel's scroll position resetting on an unrelated update, and
repeated Back walking off the end of its history, are moot rather than
fixed: the panel, its scrolling, and Back all leave with the stick and
the detail panel that used them.

**Consequences.** The surface is smaller than the one design and code
had just converged on, and the work that built the removed controls
out is not wasted so much as shelved: nothing here deletes the
reasoning in [CONTROL_MAPPING.md](CONTROL_MAPPING.md) for what each
control was going to do, it marks that reasoning superseded and gets
the accessibility and UI documents to agree with what actually ships.
Concretely superseded: [ADR-001](#adr-001)'s decision to keep the
command keys, the stick, the dial, and push-to-talk as controls
(its framing of the status board itself as the thing worth keeping
stands); [ADR-013](#adr-013)'s kind discriminator driving which
command key lit, since there is no command key left for it to drive
(the `kind` field itself is untouched at the protocol level, for
whatever control uses it next); and [ADR-019](#adr-019)'s tile budget
and its two corner badges, since a row has neither (the child ledger
gate it also recorded, that `COMPLETE` waits for the ledger to empty,
is untouched). [ADR-010](#adr-010) (no speech recognition in this
repo) is moot rather than reversed: there is no talk control left to
delegate from. [ADR-024](#adr-024) is narrowed further, not
superseded: what it recovers is unchanged, only the shape of what it
recovers into (an unbounded list, not six fixed slots) and the cadence
it runs on (continuous, not cold-start-only). ADR-008, ADR-025,
ADR-026, and ADR-027 stand exactly as written.

The gain is that a session in any repo can now actually appear on the
board once the shim is registered somewhere that reaches it, which the
six-slot design never delivered regardless of how the tiles looked;
that registration itself, at user level rather than per-repo, is
tracked as open work in `TODO.md`. The control mapping, the UI spec,
the architecture document's binding and enumeration sections, the
accessibility document's references to the removed controls, the
roadmap, and the task list are updated in the same change.

---

<a id="adr-029"></a>
## ADR-029: Taller rows, header counts, a bundled typeface

Date: 2026-09-13

**Context.** [ADR-028](#adr-028) cut the surface down to a session list
earlier the same day, but the list itself still read as a spreadsheet: 10
px labels, a single 4 px coloured edge carrying the whole state signal,
and only three of the surface's seven states, needs input, error, and the
child-ledger-gated complete, tinted at all. Nothing on the header said how
many sessions needed attention without reading every row. The owner
reviewed three mockups, Keycaps, Triage, and Large print, and picked a
combination: Triage's header counts on top of Large print's taller,
two-line rows.

**Decision.** Rows grow to a fixed 64 px, no longer a 48 px minimum, and
carry two lines: the session name, 17 px bold, over the state word in
caps, with a 30 px glyph on the left. Every coloured state now tints the
row's background toward its colour instead of only the edge: idle 6%,
thinking and complete 14%, needs input and error 22%. Unknown and ended
get a dashed outline instead of a tint. A selected row gets a 3 px inset
outline in the text colour, on top of whatever the state already draws. A
Reveal miss note moves into its own column at the right of the row, up to
3 lines, instead of overlapping the state word.

The header grows to match, 64 px (was 56), and gains a read-only summary
between the drag grip and the two controls: one pill per state that
currently has at least one session, each pairing that state's glyph and
colour with a count, in a fixed order (waiting on you, error, thinking,
complete; idle, unknown, and ended stay row-only). The summary reports;
it does not select or filter, so the header still has exactly two
controls, Move and Quit, unchanged. The window's initial height in
`tauri.conf.json` follows, 128 px (was 104), matching one header plus one
row at the new constants.

The surface's type changes from the plain Segoe UI system font to
Atkinson Hyperlegible Next, bundled rather than left to whatever happens
to be installed, because it is drawn for low-vision reading. Only the
latin subset ships, one variable-weight woff2 at
`app/ui/fonts/AtkinsonHyperlegibleNext-latin.woff2`, licensed SIL OFL 1.1
(`app/ui/fonts/OFL.txt`); anything outside that subset falls back to
Segoe UI, as before.

The empty list's text changes from "No sessions" to "Watching for
sessions," to read as a state rather than a failure.

**Context, continued.** A gap surfaced once the taller rows above were
running against a real restart: every session that was open but quiet came
back as `UNKNOWN`, identical to a session that had gone silent mid-turn,
and a board with several sessions open read as one wall of grey with no
way to tell which rows might really be waiting on the owner.

**Decision, continued.** The daemon's `Session`
(`app/src-tauri/src/state.rs`) gains `heard: bool`: false at creation,
whether the session is bound by `claude agents --json` enumeration or
restored from disk, and set true on any hook event received in this run.
`stateWord()` in `app/ui/src/format.ts` reads it: an unknown row says "not
heard yet" while `heard` is false, and "unknown" once it has been heard
from and then gone quiet past `T_unknown`. The state, the colour, and the
glyph are the same in both cases; ADR-008 is unchanged, only the row's own
word now splits the two roads into it. Unknown rows also dim, short of
ended: glyph and state word go to 75% grey toward the background, and the
name drops from bold to regular weight at 72% text colour. Deliberately
brighter than ended, because a session that has not been heard from since
Deckhand restarted may still be one that is really waiting on the owner.

**Consequences.** A 64 px row shows fewer sessions before the list
scrolls than a 48 px one did; the owner accepted that trade for
legibility. The bundled font is a new binary asset and licence file
committed to the repository, not a new runtime dependency; it is a
static asset with no network or execution behaviour, so nothing about it
moves the trust boundary in [SECURITY_MODEL.md](SECURITY_MODEL.md).
`ROW_H_LOGICAL` and `HEADER_H_LOGICAL` in `app/src-tauri/src/window.rs`
and the CSS that draws the row and header are now pinned together by
`styles.test.ts`, so one can no longer drift without the other;
`format.test.ts` covers `summaryCounts`, the pure helper behind the
header pills. [UI_SPEC.md](UI_SPEC.md), [ACCESSIBILITY.md](ACCESSIBILITY.md),
[ARCHITECTURE.md](ARCHITECTURE.md), `CHANGELOG.md`, and `TODO.md` are
updated to match in the same change. ADR-008's six colours and their
meanings are unchanged; this entry only changes how much of each row
they cover.

`heard` is tested in `state.rs`; `stateWord()`'s two outcomes are tested in
`format.test.ts`. It is a daemon bookkeeping field surfaced only as a word
and a dimming rule, not a new state and not a new colour, so
[ADR-008](#adr-008) and the session state machine in
[ARCHITECTURE.md](ARCHITECTURE.md#the-session-state-machine) are unchanged;
[UI_SPEC.md](UI_SPEC.md) and [ACCESSIBILITY.md](ACCESSIBILITY.md) are
updated to match in the same change.

---

<a id="adr-030"></a>
## ADR-030: A Hide grey toggle joins the header

Date: 2026-09-13

**Context.** [ADR-029](#adr-029)'s `heard` bookkeeping means any session
bound by enumeration or restored from disk after a restart renders
`unknown`, "not heard yet," until a hook actually arrives for it. With
several repos now registered at the user level, that grey can be most of
the list on a fresh start, and [ADR-028](#adr-028) left the header at
exactly two controls, Move and Quit, with nothing to cut that noise
besides waiting for hooks to arrive. The owner reviewed one alternative,
folding every grey row into a single expandable row, and did not choose
it: a plain show or hide toggle is simpler and leaves every row a real,
individually clickable target once it is shown again.

**Decision.** The header gains a third control, Hide grey, ordered: drag
grip, the read-only state-count summary ([ADR-029](#adr-029), unchanged),
Hide grey, Move, Quit. This supersedes only the "exactly two controls"
line in [ADR-028](#adr-028)'s decision and its counterpart in
[CONTROL_MAPPING.md](CONTROL_MAPPING.md); everything else ADR-028
decided, the list itself, auto-binding, and Move and Quit themselves,
stands.

A single click flips `Registry.hide_unknown`. Off, the control reads
"Hide" and sits unpressed. On, it shows pressed, a 2 px inset outline
echoing the row selection treatment, and its label switches to "Show N,"
where N is the count of rows currently in the `unknown` state, both "not
heard yet" and past `T_unknown`. Hidden sessions are always counted,
never silently gone. If every bound session is hidden, the list shows one
placeholder row, "N grey hidden," instead of the normal list or the
"Watching for sessions" empty state. The header's own state-count pills
are computed before this filter and do not change when it is toggled. The
control's glyph is the same grey question mark [ADR-008](#adr-008)
already draws for the unknown state, so the toggle reads as "the grey
one" without needing its label read first.

The daemon owns the setting, not the surface: `Registry.hide_unknown` in
`app/src-tauri/src/registry.rs`, persisted in a new `settings.json` in
the same data directory as `window.json` and `bindings.json`. A missing
field or a corrupt file loads as `false`. The daemon sends the current
value to the surface as `hideUnknown` on every snapshot; the snapshot
itself is unchanged otherwise and still lists every bound session
regardless of the setting, so the filtering is a rendering choice, not a
narrower observation. `toggle_hide_unknown` is the Tauri command the
surface calls on a click. The window's height, computed in
`app/src-tauri/src/window.rs`, now follows the visible row count rather
than the bound count, through a new `visible_row_count`, and the
`T_unknown` watchdog
([ARCHITECTURE.md](ARCHITECTURE.md#liveness-by-open-operation)) that can
move a session into `unknown` while the toggle is on now routes through
that same resize path, so a session timing out into a hidden row shrinks
the window exactly as toggling the control would.

Header geometry: the drag grip narrows to 14 px (was 18), Move and Quit
narrow to 48 px wide (were 56), Hide grey is 56 px wide, and all three controls are at least 44 px
tall, holding the [ACCESSIBILITY.md](ACCESSIBILITY.md#targets-and-sizing)
floor. The count pills are tightened to make room in the fixed header
width.

**Consequences.** The accepted trade-off: because `heard` resets on every
daemon restart ([ADR-029](#adr-029)), a session that was genuinely
waiting on the owner before the restart also renders `unknown` until the
owner acts in it and a hook fires, indistinguishable here from a session
that simply has nothing to report. Hiding grey can therefore hide a
session that needs a human. The "Show N" count is the mitigation, not a
fix: it says how many rows are hidden, never claims that none of them
matter, and a hidden session's row reappears, shifting every target
below it, the moment that session speaks and its state moves off
`unknown`. The owner accepted this knowingly on 2026-09-13 in exchange for
a quieter list on a multi-repo restart.
[CONTROL_MAPPING.md](CONTROL_MAPPING.md), [UI_SPEC.md](UI_SPEC.md),
[ACCESSIBILITY.md](ACCESSIBILITY.md), [ARCHITECTURE.md](ARCHITECTURE.md),
`CHANGELOG.md`, and `TODO.md` are updated to match in the same change.
ADR-008's six colours, ADR-027's
click-to-select-and-raise, and ADR-029's row and header sizing are all
unchanged; this entry only adds a third header control and a filter on
top of what they already draw.

---

<a id="adr-031"></a>
## ADR-031: Move removed, the header redrawn as a drag bar

Date: 2026-09-13

**Context.** [ADR-028](#adr-028) kept Move, a click-to-place command
that cycled the window through six edge presets, specifically because
[ACCESSIBILITY.md](ACCESSIBILITY.md) forbids drag as a required path;
the code comment on `cycle_position` called it "the route that must
always exist" alongside dragging. Reviewing the header after
[ADR-030](#adr-030) landed a third control on it, the owner asked for a
plainer surface and said directly that a Move button was not needed.
The owner is also the mouse-only user the drag rule protects, so this
trades away that rule's own guarantee, deliberately and with their
authorization, not by oversight. Separately, the header still read as
busy for what a title bar does: a striped drag grip that did nothing
but drag, a three-word Hide grey toggle whose pressed state was an
inset outline easy to miss at a glance, and Quit carrying a redundant
text label next to its glyph.

**Decision.** Move and the `cycle_position` command it drove are
deleted outright, not superseded by another control; the window is
repositioned only by dragging from here on. The whole header becomes
the drag region (`data-tauri-drag-region` on `#header` itself, no
separate grip), so any empty part of the bar drags the window; the
state-count pills sit on top of it with `pointer-events: none` so a
drag started on a pill still drags rather than being swallowed. The
header shrinks to 52 px (was 64) with 4 px padding (10 px on the
left), and its order becomes: the read-only state counts on the left,
the grey toggle, then Quit pinned to the right edge. Quit is a 44 by
44 px button carrying only its glyph, a cross, `aria-label="Quit
Deckhand"`, no visible text. The window's initial height in
`tauri.conf.json` follows, 116 px, one 52 px header plus one 64 px
row.

The grey toggle is relabelled to say what it acts on rather than a
bare count: "Hide unknown" while unknown rows show, "Show N unknown"
while they are hidden ("Show unknown" when N is 0), computed by a new
`greyLabel()` helper in `app/ui/src/format.ts`, tested in
`format.test.ts`. Its pressed state changes from a 2 px inset outline
to a lighter background and brighter text, matching how the header's
other buttons read pressed. The toggle now disappears entirely,
rather than sitting there reading "Hide unknown" with nothing to
hide, when there are no unknown sessions and hiding is already off;
Quit's position at the right edge does not move when it does, since
Quit is anchored to the edge, not to a fixed slot in a row of
controls. The all-hidden placeholder row's text changes to "N unknown
hidden" (was "N grey hidden"), matching the toggle's own wording.
Header buttons generally become plain text buttons, 44 px minimum
height, 12 px side padding, 6 px corner radius, 14 px bold text,
rather than the stacked glyph-over-label layout ADR-029 and ADR-030
drew; Quit alone stays icon-only at 44 by 44 px.

Two changes land on the row itself, orthogonal to the header. First,
the dashed outline ADR-029 gave unknown and ended rows is removed;
the two states now read apart from a live row by shape and dimming
alone, both already in place from ADR-029: unknown's question-mark
glyph and its name dropping to regular weight, ended's dash glyph and
its own dimming. `styles.test.ts` now pins the regular-weight name
and the ended dimming instead of asserting a dashed outline. Second,
a Reveal miss note moves off its own multi-line side column, which
had been squeezing the session name, clipping the state word, and
growing some rows past the fixed 64 px, and onto the row's second
line, one line, to the right of the state word. The daemon's full
sentence is shortened for that space by a new `revealNote()` helper
in `format.ts`: "No window found," "Windows blocked it," or "No
session," tested alongside `greyLabel()`.

This supersedes: [ADR-028](#adr-028)'s decision to keep Move as the
header's click-to-place alternative to dragging (the header no longer
has two controls, Move and Quit; it has a grey toggle, when there is
something to hide, and Quit); and [ADR-030](#adr-030)'s toggle
wording ("Hide" and "Show N"), its pressed style (a 2 px inset
outline), and its header geometry (a 14 px drag grip, 48 px Move and
Quit, a 56 px Hide grey, 64 px total). Everything else either entry
decided, the auto-binding list, click-to-select-and-raise, the
state-count summary and its fixed order, the daemon owning
`hide_unknown` in `settings.json`, stands unchanged.

**Consequences.** The accepted cost is exactly what
[ACCESSIBILITY.md](ACCESSIBILITY.md) exists to prevent: dragging is
now the only way to reposition the window, and a pointer user who
cannot sustain a drag, the user the forbidden-interactions table
protects, has no click-based route left to move it. A saved position
is still restored and clamped into the currently connected monitors'
work area at startup ([ADR-028](#adr-028)), so the window is never
stranded off every screen, but it cannot be nudged from wherever that
leaves it without a drag. This is recorded as an owner-approved
exception, not as compliance: the owner asked for it directly on
2026-09-13, is the person the rule protects, and accepted the
trade-off knowingly. It is an open accessibility gap, not a closed
one; a click-based reposition control, if one returns, would need its
own ADR and would close it. The drag rule in
[ACCESSIBILITY.md](ACCESSIBILITY.md) is not reworded for any other
control by this entry, only carved out for this one, named case.

[CONTROL_MAPPING.md](CONTROL_MAPPING.md), [UI_SPEC.md](UI_SPEC.md),
[ACCESSIBILITY.md](ACCESSIBILITY.md), [ARCHITECTURE.md](ARCHITECTURE.md),
`README.md`, `CLAUDE.md`, `CHANGELOG.md`, and `TODO.md` are updated to
match in the same change. ADR-008's six colours, ADR-027's
click-to-select-and-raise, and ADR-029's row and header sizing and
dimming rules are otherwise unchanged; this entry removes one
control, redraws the remaining two, and drops one visual signal, the
dashed outline, that ADR-029 added, in favour of signals ADR-029
already drew.

---

<a id="adr-032"></a>
## ADR-032: Reveal classifies the host, and a tie is a miss

Date: 2026-09-13

**Context.** The owner clicked a row and got "No window matched," the
honest-miss sentence Reveal has carried since [ADR-028](#adr-028), and
asked for research before a fix: local investigation, a web search, and
a look at the Claude Code and Codex issue trackers on GitHub.

The proximate cause was a poisoned `cwd`. `%LOCALAPPDATA%\deckhand\reveal.log`
showed the session's stored directory reading
`agent-a225d82809fcd6b14`: a subagent hook payload carries its parent
session's `session_id` but its own `cwd`, and nothing had stopped that
value from overwriting the session's real working directory the moment
a subagent ran. Every later Reveal for that session scored against the
wrong directory name and never had a chance.

The second problem was structural. Reveal's one scored match, pid,
label, and cwd text against every visible window, was built for a host
where a pid identifies one window. A plain console session's process
owns its window one to one. A Windows Terminal session shares its
owning process, `WindowsTerminal.exe`, with every other tab and window
on the machine, so a pid narrows "is this a Terminal window at all" and
nothing further. A VS Code session shares one main `Code.exe` across
every window the same way. The same score that finds a console exactly
can tie two unrelated windows, or quietly prefer the wrong one, on
either of the other two, and nothing before this entry treated a tie as
anything other than whichever candidate `EnumWindows` happened to visit
first.

**Decision.** Two fixes close the immediate bug, and a third changes
how Reveal decides at all.

The poisoning is fixed at its source and given a second line of
defence. `apply_hook` (`state.rs`) now recognises a payload carrying
`agent_id` as a subagent's, not the session's own, and skips the `cwd`
and the label derived from it; liveness still updates from the same
payload. `register_enumerated` (`registry.rs`) treats
`claude agents --json`'s own `cwd` as authoritative and corrects an
existing value that disagrees with it, rather than only filling in a
blank one, so a session poisoned before this fix, or by any gap this
entry has not found, repairs itself on the next enumeration pass.

A tie at the top score is now a miss, never resolved by pick order.
`pick_scored` (`reveal.rs`) reports whether its winning score was
unique; when two or more windows tie for the best score, Reveal returns
no match rather than the one enumeration happened to visit first. Two
equally plausible windows are exactly the case where guessing produces
a confident wrong answer, and an honest miss is what Reveal exists to
prefer over that.

Reveal now classifies the session's host before it decides how to
look, by walking the pid's parent chain (a Toolhelp32 snapshot, at most
eight hops) for the first ancestor that is `Code.exe` or
`Code - Insiders.exe` (VS Code), `WindowsTerminal.exe` (Windows
Terminal), or neither (a plain console). This is a distinction inside
what [ADR-023](#adr-023) calls the `pty` host, not a fourth value on
`SessionInfo.host`: a console and a Windows Terminal tab are both `pty`
at the protocol level, and only their window-finding strategy differs
here.

Each host gets the strategy that actually fits it:

- **Console.** `AttachConsole(pid)` followed by `GetConsoleWindow`
  gives the exact window, because the mapping is genuinely one to one;
  no title or score is involved. `AttachConsole` is process-global, so
  every call is serialised through a mutex. If the attach fails, most
  likely because the session has already exited, Reveal falls back to
  the same scored title match every other host uses as a last resort.
  This path is unverified against a real console-hosted session; it
  has only been exercised in unit tests against a fake process table.
- **Windows Terminal.** Raise the one Terminal-owned window if exactly
  one is open. If more than one is, report the honest miss, "Found
  *label* in Windows Terminal, but more than one Terminal window is
  open," rather than guess a tab. Targeting a specific tab from
  outside the process is not attempted, because there is currently no
  way to do it: `microsoft/terminal#19783`, which asked for exactly
  this (focusing a tab by its `WT_SESSION`), was closed not planned in
  January 2026, and `wt -w <id>` creates a new window rather than
  finding an existing one when the id it is given is not currently
  open.
- **VS Code.** Read every `~/.claude/ide/*.lock` file, the same lock
  files [ADR-023](#adr-023) catalogued and marked not to be built on,
  for their `pid` and `workspaceFolders` fields only; the `authToken`
  each one also carries is parsed and discarded, never logged or
  stored, and Reveal never opens the WebSocket MCP server the lock
  file advertises. The workspace folder that is the session's `cwd`
  itself, or its longest ancestor directory, wins; two folders tied at
  the same length are ambiguous and treated the same as no match, for
  the same reason as the tie rule above. On a match, Reveal resolves
  `<the owning Code.exe's own install directory>\bin\code.cmd` from
  that process's own image path (never PATH, never anything the
  session controls) and runs `code.cmd "<folder>"` hidden with a
  five-second timeout; VS Code's own CLI already focuses an
  already-open folder's window, so this one call typically does most
  of the work by itself. Reveal then restricts its own window raise to
  Code.exe-owned windows whose title contains the folder's basename,
  and counts the attempt a success only on a single title hit or a
  clean, exit-0 CLI run. No lock match, no `code.cmd` found, or a
  title match that is itself ambiguous with a nonzero or timed-out CLI
  exit, falls back to the pid-blind scored title match Reveal already
  used for this host, or reports "Found *label* in VS Code, but more
  than one matching window is open" when that fallback also ties.

  This narrows what [ADR-023](#adr-023) said about the lock files:
  they move from observed and deliberately not built on, to a
  load-bearing input for Reveal specifically. That is still consistent
  with the degrade-not-fail rule
  [CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md#interfaces-used-and-what-they-rest-on)
  holds every internal interface to: every step above has a defined
  fallback down to the pre-existing title match, so a lock file
  disappearing tomorrow costs Reveal precision on this one host, not a
  failure.

Deliberately left unwired: the extension's own session-tab link,
`vscode://anthropic.claude-code/open?session=<id>`, observed accepted
(any non-path string up to 200 characters) by reading `extension.js`
from the 2.1.270 install. Its handler, `createPanel`, reveals an
existing panel only when the receiving window's extension host already
tracks that session id in memory; any id it does not track, which is
every id today, falls through to creating a new panel that resumes the
session as a fresh process. That risks a second live copy of the
session instead of revealing the first one, for two reasons nothing
observable today rules out: a session running in VS Code's own
integrated terminal is never tracked by the extension host at all (its
immediate parent is a shell, not the extension host, which
`host::parent_is_vscode_exe` records for the log though nothing acts
on it yet), and which window a multi-window VS Code instance routes
the URI to is decided by VS Code's own core routing, outside
`extension.js`, and was not observed. Codex has the same gap for the
same shape of reason; the Codex Micro sidesteps it entirely because
every Codex thread lives in one desktop app window, which VS Code is
not.

Recorded as a risk, not mitigated: `anthropics/claude-code#77827`
reports a session where refocusing a terminal window was captured as a
click on a permission prompt already on screen there, silently denying
it. Reveal raises a window on an ordinary tile click, which is exactly
the action that issue describes, so the risk applies to Deckhand as
shipped even though nothing here approves or denies anything yet. It
is recorded now, not deferred to Phase 2, because Phase 2's approve and
deny controls would sit on this same window-raising mechanism.

**Consequences.** Reveal should miss less on the bug that prompted this
entry, since the value it was scoring against is now corrected at the
source and repaired on the next enumeration if it was not, but it also
now misses, honestly, in three places it used to guess: a tie of any
kind, more than one open Windows Terminal window, and more than one
matching VS Code window with no lock file to disambiguate it. That
trade is deliberate: an honest "no window matched" costs a second click
to check by hand, and a wrong window raised silently costs trust in
every tile after it. The console path carries a real hedge: it is
unverified against a live console-hosted session, tracked in `TODO.md`
rather than claimed here. Windows Terminal's tab targeting stays
blocked on upstream, not on anything Deckhand controls, and the
session-tab link stays unwired until a safe cross-window test exists,
both also tracked in `TODO.md`. The `anthropics/claude-code#77827` risk
is unmitigated by design for now; revisiting it belongs with Phase 2,
when a click first gains the power to deny something.

[CONTROL_MAPPING.md](CONTROL_MAPPING.md),
[ARCHITECTURE.md](ARCHITECTURE.md),
[SECURITY_MODEL.md](SECURITY_MODEL.md), and
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) are updated to match
in the same change, along with `TODO.md` and `CHANGELOG.md`.
[ADR-023](#adr-023)'s host axis, mode and capability model, and
[ADR-028](#adr-028)'s exclusion of Deckhand's own window from every
candidate list, stand unchanged; this entry only narrows how a `pty`
host's window is found and what counts as finding it.

---

<a id="adr-033"></a>
## ADR-033: A gear-triggered settings panel joins the header

Date: 2026-09-14

**Context.** The header has held exactly two controls since
[ADR-031](#adr-031): a grey toggle and Quit. Reviewing what the surface
should hold before Phase 1 closes, the owner asked for a small, fixed
set of settings reachable from the board itself rather than a config
file: whether the window stays always on top, whether Deckhand starts
with Windows, a way to put the window back where it started, and
whether the Claude Code hook wiring this machine depends on is
actually installed and current. Three lettered options were put to the
owner for where this should live; option A, an in-bar panel that
replaces the session list rather than opening a second window, was
approved. The grey toggle was reviewed alongside these and left where
it was, a one-click header control: burying a row the owner checks
often behind an extra click to open the panel first would trade away
exactly the quick access [ADR-030](#adr-030) added it for.

**Decision.** The header gains a fourth control, a gear button, 44 by
44 px minimum, reading "Settings" closed and "Close settings" open
(`gearLabel` in `app/ui/src/format.ts`), so its pressed state reads as a
different word, not only a different background colour
([ACCESSIBILITY.md](ACCESSIBILITY.md)), sitting beside the header's
existing grey toggle rather than replacing it: counts, grey toggle,
gear, Quit, left to right. A click calls a new `toggle_settings_panel`
command, which flips a daemon-side flag (`PANEL_OPEN` in `main.rs`; not
persisted, the panel always starts closed) and resizes the window
through the same path every row-count change already used,
`resize_for_rows` via `queue_resize`, sized to the panel's own row
count instead of the session list's. Counts, the grey toggle, and Quit
stay exactly where they were; only the area under the header swaps
content, in the same window, in place.

The panel holds five rows, each a full-width, 64 px row like a session
row's (`.panel-row` in `styles.css` reuses `.row`'s height with a
different two-column layout: a text label and a text state, no glyph
column, since nothing here may rely on colour alone):

1. **Always on top**, On or Off, default On. `toggle_always_on_top`
   flips and persists it (`persist.rs`'s `Settings.always_on_top`,
   defaulting to On so a `settings.json` from before this field
   existed, or none at all, loads exactly like `tauri.conf.json`'s own
   `alwaysOnTop: true`) and applies it to the real window: Tauri's
   `set_always_on_top`, then a reapplication of the `WS_EX_NOACTIVATE`
   style ([ADR-025](#adr-025)) in case toggling topmost reset it. Off
   also calls `set_skip_taskbar(false)`, since a window neither on top
   nor in the taskbar could be lost behind everything else with no way
   back; On restores whatever `tauri.conf.json` says for this window
   (`false`, unchanged). The persisted value is applied at startup too,
   before anything is visible, the same way a saved position and
   `hide_unknown` already are.
2. **Start with Windows**, On, Off, or "On (other copy)." The single
   source of truth is the registry value named `Deckhand` under
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
   (`app/src-tauri/src/runkey.rs`), read fresh on every panel open and
   every toggle and never mirrored into `settings.json`.
   `toggle_start_with_windows` takes no argument: it reads the value
   itself and either writes this exe's own quoted path over it (off, or
   pointing at a different exe) or deletes it (already pointing at this
   exe), so the daemon always decides what a click produces, never a
   client-cached guess. "On (other copy)" names the one case a plain
   toggle cannot silently resolve: some other Deckhand exe already owns
   the value, and a click repoints it at this one rather than turning it
   off. Comparison is pure and unit tested (`compare_run_value`); the
   registry calls themselves are `#[cfg(windows)]`, using the
   `windows-sys` crate already in `Cargo.toml` (`Win32_System_Registry`
   is the only added feature; no new crate, no Tauri plugin).
3. **Reset window position**, a button rather than a toggle: moves the
   window to the same default placement `setup` already falls back to
   for a first run or a stale saved position (`window::default_rect`,
   near the top-left margin of the current monitor), and persists the
   result.
4. **Hooks**, a read-only status row: "Installed," "Outdated,"
   "Missing," or "Unreadable," from a pure parse of
   `~/.claude/settings.json` against the twelve events
   `scripts/install-hooks.ps1` installs (`parse_hook_status` in
   `app/src-tauri/src/hook_status.rs`, unit tested for all four states
   plus a raw unquoted path, a leftover timeout, a partially installed
   event set, and a different tool's hook on the same event, which must
   never read as Deckhand's own). "Installed" requires every event to
   carry exactly the group the script writes today: one hook, the
   current shim path, single-quoted, no timeout. Anything short of that
   with at least one Deckhand group present is "Outdated"; nothing
   present is "Missing"; a parse failure is "Unreadable."
5. **Repair**, a button beside it: reruns `scripts\install-hooks.ps1`
   from this install's own checkout, found by walking up from the
   running exe looking for that one file (`find_repo_root` in
   `app/src-tauri/src/installer.rs`, capped at eight ancestors), via
   `powershell.exe -NoProfile -ExecutionPolicy Bypass -File <script>`,
   hidden (`CREATE_NO_WINDOW`), off the webview/event thread
   (`repair_hooks` is `async`, awaiting `spawn_blocking` exactly like
   `activate_session`'s own reveal wait), bounded at 20 seconds
   (`installer::REPAIR_TIMEOUT`) and killed if it runs longer. The hook
   status is re-read afterward and shown as the result rather than
   assumed. This is a Phase 1 dev assumption, not an installer story: it
   only works when the running exe sits inside a git checkout that
   still has `scripts/` beside it, true on the owner's machine and false
   for any distributed build. When no repo root is found, the row is not
   a native disabled button (see below); it stays styled inactive and
   its permanent state text already reads "Installer not found."

No control on this panel is a native HTML `disabled` element.
[ACCESSIBILITY.md](ACCESSIBILITY.md) forbids a control whose click does
nothing and explains nothing; the Repair row's reason not to run is
already its permanent, always-visible state text, never gated behind a
click, so a click while inactive is an honest, already-explained no-op
rather than a silent dead one. This reads slightly differently from
"disabled" as it was first put to the owner, and is recorded here as
the deliberate reading, not an oversight.

Every new command takes nothing from the webview but the click itself:
`toggle_settings_panel`, `get_settings_snapshot`,
`toggle_always_on_top`, `toggle_start_with_windows`,
`reset_window_position`, and `repair_hooks` take no argument, matching
`toggle_hide_unknown`'s existing shape, so the daemon always decides a
toggle's next state rather than trusting one the surface supplies.
None of them are `core:*` permissions, so `capabilities/default.json`
is unchanged, the same as every other custom command already registered
there is not listed.

**Consequences.** The panel adds two places Deckhand writes outside its
own data directory on an explicit click: `~/.claude/settings.json` (via
Repair, running the same installer script a person could run by hand)
and the HKCU Run key (via Start with Windows). Neither is a new kind of
trust boundary; [SECURITY_MODEL.md](SECURITY_MODEL.md)'s existing
"touch other tools' config like a guest" rule already covered
`settings.json`, and is extended in the same change to name the Run
key: Deckhand only ever reads or writes its own named value there,
never anything else under Run, only ever on a click, and only ever the
one value the panel shows. Repair is unverified against a real login
end to end (read through, not run against a live `settings.json`), and
Start with Windows is unverified against an actual Windows startup;
both are added to `TODO.md` as manual verification items rather than
claimed proven here.

This adds a fourth header control, the gear, alongside the counts, the
grey toggle, and Quit that [ADR-030](#adr-030) and [ADR-031](#adr-031)
already put there; it does not supersede either entry's header
placement, wording, or geometry for the grey toggle, which this entry
leaves exactly as [ADR-031](#adr-031) drew it. [ADR-034](#adr-034)
restyles the grey toggle itself; this entry only makes room for the
gear beside it. Everything else either entry decided (auto-binding,
click-to-select-and-raise, drag-only repositioning, the state-count
summary, Quit itself) is unaffected.

[CONTROL_MAPPING.md](CONTROL_MAPPING.md), [UI_SPEC.md](UI_SPEC.md),
[ACCESSIBILITY.md](ACCESSIBILITY.md),
[ARCHITECTURE.md](ARCHITECTURE.md),
[SECURITY_MODEL.md](SECURITY_MODEL.md), `README.md`, `CLAUDE.md`,
`CHANGELOG.md`, and `TODO.md` are updated to match in the same change.

---

<a id="adr-034"></a>
## ADR-034: Visual refresh of the header, settings panel, and rows

Date: 2026-09-14

**Context.** A screenshot of the settings panel [ADR-033](#adr-033)
shipped named several problems at once: the gear opened with a text
button reading "Close settings" and Quit was a literal `&#10005;`
entity rather than a drawn icon; every setting showed its value as
plain top-aligned "On"/"Off" text instead of a recognisable control,
next to which the header's own grey toggle, still [ADR-031](#adr-031)'s
plain text button, now read like a leftover; the window did not size
itself to the panel's real content, so a native grey scrollbar appeared
on a panel that should have fit exactly; "Reset window position"
wrapped across three lines with its description crowding the label;
Hooks and Repair sat as two separate, visually redundant rows; and the
panel read as one flat list with no grouping. The owner approved scope
B for the fix: the header, the settings panel, and the session list
rows, recorded here as one entry since the row and panel restyle share
the same design tokens.

**Decision.** Four things change together.

*Header icons.* The gear and Quit become inline svg icon buttons
(`app/ui/src/icons.ts`, hand-drawn in the same minimal stroke style as
the session-state glyphs in `format.ts`, not copied from Lucide or any
other icon set). The gear keeps `aria-label="Settings"` and
`aria-pressed` exactly as ADR-033 defined them, but its open/closed
state is now carried by a shape swap, a cog closed and an arrow back
to the session list open (`gearIconKind` in `format.ts`), on top of a
raised, tinted background, so the pressed state is still more than one
colour change ([ACCESSIBILITY.md](ACCESSIBILITY.md)). `gearLabel`, the
text-based version of this signal, is removed as dead code along with
it.

*Toggle switches.* In the panel, Always on top and Start with Windows
render as a track-and-thumb switch beside their existing On/Off (or
tri-state) word, never replacing it
([ACCESSIBILITY.md](ACCESSIBILITY.md): colour, and now shape, is
never the only channel). The header's own grey toggle gets the same
switch, reusing the identical `.switch`/`.switch-thumb` graphic and
`role="switch"`/`aria-checked` pattern rather than a second copy of it,
and its wording shortens to match: "Hide grey" while unknown rows show,
"N hidden" while they are hidden (`hideGreyWord` in `format.ts`,
replacing [ADR-031](#adr-031)'s "Hide unknown" / "Show N unknown"),
since a switch already carries the on/off distinction the longer
wording existed to spell out. In every case the switch is not a
control nested inside the row's own `<button>`; the row (or, in the
header, the button itself) carries `role="switch"` and `aria-checked`,
and the track and thumb are purely decorative, `aria-hidden`.
`startWithWindowsChecked` in `format.ts` maps the tri-state value to a
boolean check (both "on" shapes read checked; the word beside the
switch is what still tells them apart).

*Reset position and the combined Hooks row.* "Reset window position"
becomes a two-line action row labelled "Reset position," with the
existing confirmation text (`resetPositionText`, unchanged) as a
small description line underneath and a decorative return-arrow icon
on the right; the row itself, not a second nested control, stays the
click target. Hooks and Repair merge into one row: a status pill
(`hookStatusPillClass` in `format.ts`, tint plus the existing
`hookStatusText` word, never colour alone) sits beside the "Hooks"
label, a real `<button>` labelled "Repair" sits on the right at the
44 px floor, and Repair's own result becomes a secondary line
(`repairSecondaryText`, replacing `repairRowText`: empty while idle,
since the button's own label already says "Repair," and otherwise the
same "Running...", "Repaired," "Timed out," or "Failed to start" text
ADR-033 defined). Because the Hooks row itself is a plain, non-button
div exactly as ADR-033 already had it, nesting a real button inside it
is valid; `repairButtonInactive` carries forward ADR-033's rule
unchanged, styled inactive rather than natively disabled, and still
clickable with its reason already visible.

*Grouped sections.* The panel's rows sort into two titled cards: Window
(Always on top, Start with Windows, Reset position) and Claude Code
(the combined Hooks row); the header's own grey toggle, restyled above,
never enters the panel, so there is no third, List, card. Four rows
across two sections, down from five flat rows, each still the
64 px floor [ADR-029](#adr-029) set and
[ACCESSIBILITY.md](ACCESSIBILITY.md) requires, never shrunk.

*Window sizing.* The scrollbar bug was a real mismatch: the daemon
sized the open panel at a flat `PANEL_ROW_COUNT` (5) times
`ROW_H_LOGICAL`, a formula built for the session list's one-row-per-
session shape, and the panel's fixed content, now two section
titles and their gap on top of four rows, no longer fit it. Rather
than hand-tune that constant again, `window.rs` gains a dedicated,
fixed formula for the panel: `PANEL_ROW_COUNT` (4), `PANEL_SECTION_COUNT`
(2), `SECTION_TITLE_H_LOGICAL` (32), `SECTION_GAP_LOGICAL` (12), and
`PANEL_PADDING_LOGICAL` (8) sum in `panel_content_height_logical()`,
and `panel_window_height()` adds the header and clamps to the monitor's
work area exactly as `window_height()` already does for the session
list. `main.rs` routes to it through the same `resize_for_rows` /
`queue_resize` path every other resize already uses
("the existing resize path," per ADR-033's own framing): a new
`window::PANEL_SENTINEL` (`usize::MAX - 1`, distinct from
`LAST_ROW_COUNT`'s own "never sized yet" sentinel at `usize::MAX`)
stands in for a real row count wherever the panel is open, and
`resize_for_rows` branches on it instead of computing
`row_count * ROW_H_LOGICAL`. `styles.test.ts` pins the three new pixel
constants against the matching CSS rules
(`.settings-section-title`, `.settings-section + .settings-section`,
`#list.panel`) the same way it already pins `ROW_H_LOGICAL` and
`HEADER_H_LOGICAL`, so the two cannot drift apart silently again. The
session list's own sizing is untouched: it still fits `#list` with no
padding or gap, exactly `rows * ROW_H_LOGICAL`, which is also why its
own "rows as cards" look (below) is built entirely from borders inside
each row's existing 64 px box rather than any spacing on `#list`.

*Modern look, session rows included.* Every row, session and panel
alike, gains rounded 10 px corners, a 4 px left accent bar in the
row's own `--ring` (the same per-state colour already driving the
background tint, so this is a second reading of an existing token,
not a new one), and a subtle inset top highlight standing in for
elevation. The background tint itself softens now that the accent bar
carries part of the signal: idle 5% (was 6%), thinking and complete
8% (was 14%), needs input and error 14% (was 22%); unknown and ended
stay untinted as ADR-031 left them. The "cards with a small gap" look
on the session list is drawn entirely with each row's own 6 px
ground-coloured bottom border (was 2 px) inside its existing
border-box 64 px height, never a vertical gap or padding on `#list`,
so the window-sizing formula above is untouched. Both rows and panel
cards do gain a small horizontal inset from the window's left and
right edges, 8 px, the same figure `PANEL_PADDING_LOGICAL` already
uses for the panel's own outer padding: a base rule on `#list` gives
the session list this inset in list mode, and `#list.panel`'s own
padding already carries the identical 8 px horizontally, so the two
agree without either overriding the other; horizontal only, so it
still never touches the session list's height. In the panel, a switch
row's state word sits to the left of its switch, right-aligned to the
switch's own left edge (`.switch-word`'s existing min-width and
right-aligned text, `renderSwitchRow` in `main.ts` ordering word before
switch), so every row's trailing control, a switch, the Reset row's
icon, or Repair, ends flush against the row's own right edge. Header
count pills gain a
1 px tinted border. Header buttons and rows gain a 120 ms background
transition, `:focus-visible` gets an explicit outline (the surface has
never removed the browser default, but never styled it either), and
`prefers-reduced-motion: reduce` now also turns off the row, switch,
and header-button transitions, on top of the spinner override
ADR-029 already had. Every transition here is at or under 150 ms.

**What stays frozen.** [ADR-008](#adr-008)'s six state colours and
their meanings are unchanged; the accent bar and softened tint reuse
the exact same `--ring` value each row already resolved, so no session
state's hue or mapping moves. The Hooks status pill borrows the same
green/amber/red tokens for a loosely matching but distinct meaning,
hook-install health rather than session state, which is a reuse of
the palette, not a new entry in ADR-008's table. The 64 px row floor
([ADR-029](#adr-029)), the 44 px hit-target floor
([ACCESSIBILITY.md](ACCESSIBILITY.md)), the bundled Atkinson
Hyperlegible Next typeface, the header's drag-only repositioning
exception ([ADR-031](#adr-031)), and the "styled inactive, never
natively disabled" rule for a control with nothing to do
([ACCESSIBILITY.md](ACCESSIBILITY.md), first applied to Repair by
[ADR-033](#adr-033)) all carry forward unchanged.

**Consequences.** The panel's height is now hand-maintained in two
places, `window.rs`'s five constants and the matching `styles.css`
rules, instead of one row count falling out of the row-per-session
formula for free; `styles.test.ts`'s new cross-file assertions are
what keep a future edit to one from silently reintroducing the
scrollbar bug in the other, the same safety net ADR-029 built for
`ROW_H_LOGICAL` and `HEADER_H_LOGICAL` before it. Nesting a real
`<button>` (Repair) inside a plain, non-button row (Hooks) is a
pattern this entry establishes rather than one ADR-033 needed; a
future row that needs both a status display and its own action can
reuse it instead of inventing a new one. This is a visual and markup
change only: no new observation capability, no change to what the
daemon can read or write, and no change to
[SECURITY_MODEL.md](SECURITY_MODEL.md)'s trust boundary.

[UI_SPEC.md](UI_SPEC.md), [ACCESSIBILITY.md](ACCESSIBILITY.md),
[CONTROL_MAPPING.md](CONTROL_MAPPING.md), `README.md`, `CHANGELOG.md`,
and `CLAUDE.md` (next ADR bumped to 035) are updated to match in the
same change.

<a id="adr-035"></a>
## ADR-035: Liveness from the process, state from the scan

Date: 2026-09-15

**Context.** Two failures showed up in dogfooding. A session left idle for
fifteen minutes turned grey, and rows disappeared from the list after a
while with nothing having ended. With the grey toggle on, a greyed row is
also a hidden row, so the two symptoms may have been one. Both trace to
the same gap: the daemon has no proof that a session's process is alive.
It has hook events, which stop arriving the moment a session has nothing
to do, and the periodic `claude agents --json` scan, which it has treated
as a source of pids and labels only.

The greying came from `T_unknown` ([ADR-016](#adr-016)): fifteen minutes
with no hook event moves any session to `unknown`, measured from the last
event of any kind and suspended by nothing. That rule was written for a
runtime that could die without a trace, and it cannot tell a session
waiting for its owner from one whose terminal was killed. The
disappearances came from the membership rule in [ADR-028](#adr-028): a
bound session that a successful scan omits, and that has had no hook for
sixty seconds, is dropped. Whether the rows the owner lost were omitted by
the scan or had really exited, the daemon could not have told the
difference, and it logs neither.

Meanwhile the scan's output shape changed again. [ADR-024](#adr-024)
narrowed [ADR-017](#adr-017) because 2.1.220 returned no `status` key, and
named its own reopen condition: "It reopens if a release adds a
status field to the enumeration, at which point the conditional in step
2 starts firing on its own." On the installed 2.1.270 every row carries
`status`. The values
`busy` and `idle` were observed live on 2026-09-15; `shell` and `waiting`
sit beside them in the CLI's own validator list and have not been
observed. The registry the command reads,
`%USERPROFILE%\.claude\sessions\<pid>.json`, also carries `procStart`, a
process identity the CLI checks before listing a row, so a session the
scan lists has a live process at the moment of listing. TODO.md has
carried the fix since [ADR-032](#adr-032): hold a process handle per
session, flip to `ended` when it exits, and stop greying a live idle
session.

**Decision.** Two channels, each doing only what it can prove.

1. *Liveness from the process.* When a scan reports a pid for a session,
   the daemon opens a handle on it with `OpenProcess(SYNCHRONIZE)` and
   keeps it for the life of the session. The daemon's existing two-second
   tick thread asks the handle, with a zero-timeout wait, whether the
   process has exited. Exit
   without a `SessionEnd` moves the session to `ended` and off the list,
   exactly as `SessionEnd` would; only a `SessionStart` revives it. A
   held handle also stops Windows from reusing the pid, so no start-time
   check is needed. A session with a live handle is never dropped because
   a scan omitted it; the sixty-second prune now applies only to sessions
   the daemon holds no handle for (no pid known, or the handle could not
   be opened). A handle is opened from a scan sighting only, never from a
   pid restored from disk, since a restored pid may already name another
   process.

2. *State from the scan, when hooks have not spoken.* The scan's `status`
   colours a session only while hooks have not: when the session is
   `unknown`, or has never been heard from by a hook in this run. The
   mapping is `busy` and `shell` to `thinking`, `waiting` to
   `needs_input`, `idle` to `idle`; any other value leaves the state
   alone. Once a hook has coloured a session the scan never recolours it:
   hooks carry what the scan cannot (green means finished and unread,
   amber carries the question, red carries the error), and a coarse
   `idle` must not erase them. The scan never produces `complete`.

3. *`T_unknown` narrowed.* For a session with a live handle, silence is
   not degradation: `idle`, `complete`, `error`, and `needs_input` hold
   for as long as the process lives. The one exception is `thinking`: a
   turn in flight produces hook events, so a `thinking` session with no
   hook for `T_unknown` whose latest scan status is not `busy`, `shell`,
   or `waiting` moves to `unknown`, because the two channels disagree and
   the colour cannot be trusted. For a session without a handle the old
   rule stands with one change: being listed by a successful scan counts
   as an event of any kind, so the fifteen minutes run from the later of
   the last hook and the last sighting.

Process death without `SessionEnd` is `ended`, not the `error` that
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) had pencilled in. Red
with no exit path would sit on the list until the daemon restarted, since
no further event can arrive for a dead process, and a crash is already
visible in the host window the row points at. This closes the open
question in [ARCHITECTURE.md](ARCHITECTURE.md) on confirming process
death cheaply enough to poll: a held handle and a zero-timeout wait cost
microseconds per session per tick.

**Consequences.** After a daemon restart the scan colours every live
session within one rescan instead of leaving the list grey, the failure
[ADR-017](#adr-017) originally claimed to fix and [ADR-024](#adr-024) had
to take back. An idle session stays white for as long as its process
lives. A row leaves the list on `SessionEnd`, on process exit, or, for a
session with no handle, on the existing scan-plus-sixty-seconds rule. The
`unknown` state keeps its meaning, observation degraded and never a
guess; what changes is that a live process waiting for its owner is no
longer called degraded.

The known limit: if the hook channel breaks mid-session while the process
lives, a session parked in `idle`, `complete`, `error`, or `needs_input`
keeps that colour until a hook or the process exit moves it. The settings
panel's hooks status row is the place to notice a broken channel. The
transcript fallback in TODO.md remains the planned answer for missed
individual events and is not changed here.

ADR-024's conditional (`busy` to `thinking`, kept in the spec at no cost)
now fires: [ADR-024](#adr-024) is superseded on that point and stands on
the rest. `list_sessions` keeps its `documented` confidence, now observed
against 2.1.270 as well, and nothing beyond the mapping above depends on
the key being present. [ADR-016](#adr-016) is superseded on the
`T_unknown` rule as stated in point 3. The 2.1.220 stamp on
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) stays partial; the
`status` observation carries its own 2.1.270 stamp.
[ARCHITECTURE.md](ARCHITECTURE.md),
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md), `TODO.md`,
`CHANGELOG.md`, and `CLAUDE.md` (next ADR bumped to 036) are updated in
the same change.

<a id="adr-036"></a>
## ADR-036: The scan breaks ties when hooks fall silent; the transcript fallback is retired

Date: 2026-09-15

**Context.** [ADR-035](#adr-035) gave the scan two jobs, liveness through a
process handle and colour for a session no hook has coloured, and drew one
line: once a hook has coloured a session, the scan never recolours it. That
line left one gap. A `Stop` or `PostToolUse` that never arrives leaves a
session blue while the CLI sits idle, and nothing corrects it until
`T_unknown` turns it grey fifteen minutes later; a lost `UserPromptSubmit`
leaves a green or white row while a turn runs. The planned answer since
Phase 0 was the transcript fallback: tail the session's JSONL under
`%USERPROFILE%\.claude\projects\` and infer the turn boundary from it.
TODO.md carried its two halves, the trigger condition and the parser, and
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md) already rated the
per-line schema "internal, changes between versions" and the channel a
last resort.

Two things changed the trade. The scan now carries `status`, observed on
2.1.270, and reports `idle` within one fifteen-second rescan of a turn
ending, hooks or no hooks; it is the same signal the transcript would have
been read for, from a documented command rather than an undocumented file.
And no missed hook has been observed in dogfooding: ten of the twelve
documented events have fired with full payloads, and the two unobserved
ones (`Notification`, `StopFailure`) are not the ones a fallback would
recover. This session's own transcript is already 1.7 MB. Building a
tailing parser against a format the adapter doc itself calls fragile, for
a failure not yet seen, is the wrong trade.

**Decision.**

1. *The tie-break.* The scan may recolour a hook-coloured session in
   exactly two cases, and only after two consecutive scans contradict the
   hook-set colour with no hook event between them (about thirty seconds
   at the fifteen-second rescan): a `thinking` session the scan reports
   `idle` moves to `idle`, with its open operations and child ledger
   cleared, since a session the CLI calls idle has nothing in flight; an
   `idle`, `complete`, or `error` session the scan reports `busy` or
   `shell` moves to `thinking`, as a lost `UserPromptSubmit` would have
   done. Any hook event resets the count. The scan still never produces
   `complete`, still never touches `needs_input`, and `waiting` never
   triggers the tie-break in either direction. ADR-035's immediate
   colouring of a never-heard or `unknown` session is unchanged, and its
   fifteen-minute rule for a `thinking` session the scan does not confirm
   busy stands for the case where the scan carries no status at all.

2. *The transcript fallback is retired.* Deckhand does not read session
   transcripts. Hooks and `claude agents --json` are the only observation
   channels; the `transcript_path` field hooks carry stays unused. Both
   TODO items close as retired, not as done. Should a missed hook ever be
   observed that the tie-break does not recover within a rescan, that
   observation, not this decision, is the grounds to reopen.

**Consequences.** A lost `Stop` costs about thirty seconds of wrong blue
instead of fifteen minutes of wrong blue and then grey. A lost
`UserPromptSubmit` costs about thirty seconds of stale white, green, or red
instead of a whole turn. What the tie-break does not recover self-heals on
the next hook: a `needs_input` whose answering `PostToolUse` was lost turns
blue at the turn's next `PreToolUse`. Green stays a hook-only claim, so a
turn whose `Stop` was lost shows white, not green; the finished-and-unread
signal is lost with the hook, honestly, rather than reconstructed.

The privacy position in [ARCHITECTURE.md](ARCHITECTURE.md) tightens from
"transcripts are read where they already are" to "transcripts are not
read". [ADR-035](#adr-035) is superseded on the one word "never" in its
second point and stands on the rest. [ARCHITECTURE.md](ARCHITECTURE.md),
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md),
[EXECUTIVE_SUMMARY.md](EXECUTIVE_SUMMARY.md), `TODO.md`, `CHANGELOG.md`,
and `CLAUDE.md` (next ADR bumped to 037) are updated in the same change.
