# Roadmap

Deckhand is built in phases. Each phase should leave the project in a state
that is honestly describable, even if that description is "half the surface
exists and the rest is dark". Nothing ships that claims to work and does
not.

The status badge in `README.md` should always match whichever phase below is
marked current.

---

## Phase 0: Specification (current)

**Goal:** describe the system precisely enough that Phase 1 can be built
without re-deciding its architecture halfway through.

Deliverables:

- Repository scaffolding and community health files.
- `docs/CONTROL_MAPPING.md`: what every physical control does.
- `docs/ARCHITECTURE.md`: how the pieces fit together.
- `docs/ADAPTER_PROTOCOL.md`: the contract between the surface and any agent
  backend.
- `docs/CLAUDE_CODE_ADAPTER.md`: how that contract is met for Claude Code
  specifically.
- `docs/UI_SPEC.md`: the visual and interaction spec.
- `docs/SECURITY_MODEL.md`: the trust boundary, and what the approve button
  actually authorises.
- `docs/ACCESSIBILITY.md`: the mouse-only rule made concrete.
- `docs/DECISIONS.md`: the decisions already made, and why.
- `docs/EXECUTIVE_SUMMARY.md` and `docs/WORKFLOW.md`: the front door and the
  anti-drift rules.
- The two de-risking spikes, tracked in `TODO.md`: the Tauri
  no-focus-steal window, and hook payload validation against a live
  Claude Code install. These gate the start of Phase 1 (ADR-009).

**Done when** the documents above are internally consistent, a second
implementer could start Phase 1 from them without asking the author basic
questions, the open questions that remain are written down rather than
silently assumed, and both spikes have answered their questions.

---

## Phase 1: Observation only

**Goal:** prove that Claude Code session status can be inferred reliably
enough to show on a tile, before any authority is put behind it.

Deliverables:

- The Deckhand daemon (local background process, transport per
  `docs/ARCHITECTURE.md`).
- The hook shim, installed via `settings.json`, reporting `SessionStart`,
  `UserPromptSubmit`, `PreToolUse`, `PermissionDenied`, `PostToolUse`,
  `PostToolUseFailure`, `SubagentStart`, `SubagentStop`, `Notification`,
  `Stop`, `StopFailure`, and `SessionEnd` events to the daemon: twelve
  events, not seven, once the status inference fixes for failed turns,
  failed tool calls and the classifier's own denials are accounted for, and
  the child ledger and the liveness bracket both have something to read.
  `PreToolUse` is installed twice, once gating and narrow and once
  non-gating and match-all, so thirteen entries in total.
- Six agent tiles in the surface, each showing live status colour for a
  bound session.
- No write authority anywhere in this phase. It can watch and it can be
  wrong, but it cannot act on being wrong.
- A spike: observe Stop-hook `decision: "block"` behaviour against a live
  Claude Code install, including whether the turn stays in the same session
  and what ceiling, if any, exists on holding it open. This is observation
  of a mechanism, not a send capability; nothing in this phase gains a way
  to put a prompt into a running session.

**Done when** six concurrent Claude Code sessions can be watched at once
with status that stays correct, including the unbound and error states, for
a normal working session without manual correction.

---

## Phase 2: Approve and deny

**Goal:** give Deckhand its first real authority.

Deliverables:

- `PreToolUse` hook wired to return a permission decision, not just report
  one.
- Approve and deny command keys routed through the daemon back to the
  waiting hook.
- Amber status tied directly to an actual pending permission decision,
  never inferred indirectly.
- A visible, auditable record of what was approved or denied, and for which
  tool call.

**Done when** a real permission prompt can be approved or denied from the
surface, reliably, with no path by which the surface shows amber and
nothing is actually waiting.

---

## Phase 3: The rest of the surface

**Goal:** finish the physical control set.

Deliverables:

- Dial: stepping through session options, minus and plus, commit targets.
- Stick: scrolling and expanding the detail panel, and returning to the
  previously selected tile.
- The Answer key and its answer targets, and the Reveal key.
- Layer strip: profile switching.
- Settings surface.
- Theming.

**Done when** every control described in `docs/CONTROL_MAPPING.md` is
implemented, and none of them require anything but a pointer.

---

## Phase 4: Hosted mode

**Goal:** add the mode where Deckhand starts and fully drives sessions
itself, via the Claude Agent SDK, trading the normal terminal UI for full
control including sending prompts.

Deliverables:

- Session lifecycle management through `@anthropic-ai/claude-agent-sdk`.
- Send wired to hosted sessions (this is the mode where send can actually
  work end to end).
- A clear, visible distinction in the surface between an attached tile and
  a hosted tile, so it is never ambiguous which authority level applies.

**Done when** a session can be started, driven, and ended entirely from the
surface, with attached mode unaffected.

---

## Phase 5: Talk

**Goal:** add push-to-talk, delegating speech recognition to MacroVox
rather than reimplementing it.

Deliverables:

- Talk button wired to MacroVox.
- Transcribed text routed into the composed message for send.
- Graceful behaviour when MacroVox is not installed or not running.

**Done when** talk works as a thin client of MacroVox, with no speech
recognition logic living inside Deckhand itself.

---

## Phase 6: Beyond Claude Code

**Goal:** prove the adapter contract generalises by implementing a second
adapter.

Deliverables:

- A second adapter (target chosen when this phase starts).
- Any changes to `docs/ADAPTER_PROTOCOL.md` forced by the exercise of
  actually writing a second implementation.

**Done when** the second adapter runs against the same surface code as the
Claude Code adapter, with no Claude-Code-specific assumptions found leaking
through the contract.

---

## Principles

- Mouse-only or it does not ship. This is not negotiable per feature.
- Documented interfaces over fragile internals. The transcript JSONL
  fallback stays a fallback, permanently.
- Never guess a status colour. Show unknown instead of a wrong answer.
- The approve button is a security surface, not a convenience. Treat every
  change near it accordingly.
- The surface must never steal focus from whatever the user is actually
  working in.
- Ship nothing that claims to work and does not. A missing feature is fine.
  A misleading one is not.
