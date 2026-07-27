# Decisions

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
