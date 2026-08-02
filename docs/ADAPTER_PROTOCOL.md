# Adapter protocol

Status: **proposed**, version `0`. Expect it to change once a second adapter
exists, because a contract with one implementation is a guess.

An adapter connects Deckhand to one agent runtime. The daemon knows only this
contract. Everything specific to Claude Code lives behind it, documented in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

## Capabilities

An adapter declares what it can do. The surface reads these declarations and
disables controls it cannot drive, so a missing capability is a greyed-out
button that says why when you click it, never a button that silently does
nothing. The reason is revealed in the detail panel, which the same click
expands if it is collapsed. Not a tooltip: the surface never takes focus, so a
dwell or eye-tracker user has no route to one. See
[ACCESSIBILITY.md](ACCESSIBILITY.md#forbidden-interactions) and
[DECISIONS.md](DECISIONS.md#adr-021).

| Capability | Meaning | Required |
| --- | --- | --- |
| `observe_status` | Report session state changes | Yes |
| `list_sessions` | Enumerate sessions available to bind | Yes |
| `focus_session` | Raise the session's window | No |
| `decide_permission` | Answer a pending permission request | No |
| `answer_question` | Answer a question the session asked, by choosing one of its options | No |
| `send_prompt` | Put a prompt into a session | No |
| `interrupt` | Stop the current turn | No |
| `set_option` | Change a session option, for example the model | No |

Only the first two are required. An adapter that can do nothing but report
status is still useful: the status board is most of the value.

`answer_question` is optional and **unproven**. No answer channel has been
observed on any runtime, so no adapter can honestly declare it today. It is in
the table because the surface has to know whether the question in front of the
user is answerable at all, and the honest default is that it is not. MCP
elicitation is a nearby mechanism, but it is MCP-only and has not been
observed here, so it is not treated as a candidate. See
[DECISIONS.md](DECISIONS.md#adr-013).

`send_prompt` is the capability most likely to be over-declared. A channel
that can only deliver at a turn boundary cannot put a prompt into an idle
session, which is exactly when a person wants to type, so it does not satisfy
this capability. Rule 7 applies: declare `false` until an adapter has been
observed delivering.

Controls map onto capabilities, and the mapping is the whole reason the
declarations exist:

| Surface control | Capability it needs |
| --- | --- |
| Approve, Deny | `decide_permission` |
| Answer | `answer_question` |
| Interrupt | `interrupt` |
| Continue | `send_prompt` |
| Send | `send_prompt` |
| Reveal | `focus_session` |
| Dial commit target | `set_option` |

A control whose capability is `false` ships visible and disabled, and names the
missing capability when clicked. Continue and Send are the current examples:
in attached mode `send_prompt` is `false`, so both are disabled and say so
rather than looking available. The dial's steppers are a readout and need no
capability; only its commit target writes, so only the commit target is gated
on `set_option`.

Each capability also carries a **confidence**, because "supported" and
"supported by a documented interface" are different claims:

| Confidence | Meaning |
| --- | --- |
| `documented` | Built on a public interface the runtime commits to |
| `internal` | Works, but depends on something undocumented that may break |
| `synthetic` | Simulated from outside, for example by sending keystrokes at a window |

The surface must show `synthetic` differently from `documented`. A user pressing
Approve deserves to know whether that is a real API call or a best guess.

Confidence says what kind of interface a capability rests on. It says nothing
about whether anyone has watched that interface work, which is a separate
claim, so a confidence may also carry an evidence note in parentheses:
`documented (observed 2.1.220)` is a stronger statement than `documented`
alone. The evidence words are fixed and mean one thing each.

| Evidence | Meaning |
| --- | --- |
| `observed` | Run against a live install, named with the version it was run against |
| `documented` | Read from the runtime's own documentation, not seen to fire here |
| `unverified` | Neither. The default, and never dressed up as anything else |

`list_sessions` is the first entry to carry one: the Claude Code adapter
declares it `documented (observed 2.1.220)`, on the strength of a session
enumeration that was run on the author's machine. Everything else in that
adapter stays `documented` or `unverified`. See
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md#capability-declaration).

## Types

Sketched in TypeScript for readability. The real boundary is Rust.

```ts
type SessionId = string;

type SessionState =
  | "idle" | "thinking" | "needs_input"
  | "complete" | "error" | "ended" | "unknown";

// Whose decision an `ask` reaches. Runtime-specific by nature: these are the
// values the reference runtime accepts, and another adapter maps onto the
// nearest one or reports `unknown`. The six runtime values are `observed`
// against Claude Code 2.1.220: `claude --help` lists exactly this set as the
// choices for `--permission-mode`. `default` is not among them. Whether the
// settings key `permissions.defaultMode` accepts a value spelled `default`
// is unverified, and nothing here assumes either answer. Behaviour in any
// mode other than `manual` is still unverified; this is the set of names,
// not a claim about what each one does.
//
// `unknown` is Deckhand's own seventh value, not a runtime one. It is not a
// fallback for laziness: it is the honest answer when a payload carries no
// mode at all, which is not rare, and it is also the answer for a value not
// in this list.
type PermissionMode =
  | "manual" | "acceptEdits" | "plan"
  | "auto" | "dontAsk" | "bypassPermissions"
  | "unknown";

// What is holding the session's process, which is a different question from
// who started it. One adapter can span all three at once. A runtime with no
// such distinction reports `pty` if it has a window and `sdk` if it does not.
// See [DECISIONS.md](DECISIONS.md#adr-023).
type Host =
  | "pty"               // a terminal, whether its own window or an editor's
  | "vscode-extension"  // inside the editor process; no window of its own
  | "sdk";              // started by Deckhand; no window by construction

interface SessionInfo {
  id: SessionId;
  adapter: string;          // "claude-code"
  label: string;            // human-facing, usually the project directory
  cwd: string;
  mode: "attached" | "hosted";
  host: Host;
  permissionMode: PermissionMode;
  model?: string;
  startedAt: string;        // ISO 8601

  // What this session can actually do, which is not always what its adapter
  // can do. Narrower than or equal to the adapter's declaration, never wider.
  capabilities: Record<Capability, Confidence | false>;
}

interface SessionUpdate {
  id: SessionId;
  state: SessionState;
  at: string;               // ISO 8601, when the adapter observed it
  detail?: {
    kind?: "permission" | "question";  // see the note below on absent kinds
    tool?: string;          // what is running, for the tile subtitle
    question?: string;      // what is being asked, when kind is "question"
    options?: string[];     // answer labels in full, never letters or indices
    children?: number;      // open subagents; see the child ledger
    contextUsedPct?: number;
    costUsd?: number;
    error?: { kind: string; message?: string };
  };
}

// Named `Deckhand...` on purpose. `PermissionRequest` is also the name of a
// real Claude Code hook event with a different shape, and the collision is a
// trap for whoever implements this next.
interface DeckhandPermissionRequest {
  requestId: string;
  sessionId: SessionId;
  toolName: string;
  toolInput: unknown;       // shown to the user, never interpreted by the daemon
  receivedAt: string;
  expiresAt: string;        // the adapter must answer before this
}

type PermissionDecision =
  | { decision: "allow"; reason?: string }
  | { decision: "deny"; reason?: string }
  | { decision: "ask"; reason?: string };   // hand it back to the runtime
```

`permissionMode` is on `SessionInfo` because it decides what an `ask` actually
reaches, and therefore whether Approve and Deny mean anything on that session
at all. It is `unknown` whenever the runtime does not say, which is not rare:
not every payload carries one. The surface shows it as text, never as a
colour. See [DECISIONS.md](DECISIONS.md#adr-018) and, for the corrected
membership of the set, [DECISIONS.md](DECISIONS.md#adr-022).

`host` is separate from `mode` because they answer different questions and
the answers do not track each other. `mode` says who started the session;
`host` says what is holding its process, and therefore what can be done to
it from outside. A session someone else started can sit on a `pty` or
inside an editor, and those two differ on whether there is a window to
raise or keystrokes to send, while agreeing on everything hooks provide.

Capabilities are on `SessionInfo` for the same reason. One adapter can span
hosts whose capability sets genuinely differ, so an adapter-level record
cannot be true for all of its sessions at once: the Claude Code adapter
declares `focus_session` as `synthetic` on a `pty` host and `internal` on a
`vscode-extension` host simultaneously. The adapter's own record is the
ceiling, a session's is what applies, and a session may never claim more
than its adapter. The surface reads the session and nothing else, so a
control is lit by what this session can do rather than by what its runtime
can do somewhere else. See [DECISIONS.md](DECISIONS.md#adr-023).

`kind` is what makes amber usable. It separates a decision Approve can make
from a question that needs one of its options chosen, and without it the
surface has to light Approve on both. It carries no colour of its own: amber
is one colour with two kinds, and [DECISIONS.md](DECISIONS.md#adr-008) is
untouched.

An adapter reporting `needs_input` should carry a `kind`, and it is optional
because some runtimes announce a prompt without saying which sort it is.
Claude Code's `Notification` event is the known case. An adapter that cannot
tell reports no `kind` rather than picking one, and the surface then enables
neither Approve nor Answer, showing the amber and the reason instead. That is
the fail-safe direction: the cost is a click the user has to make in the
terminal, and the alternative is a guessed `kind` lighting a button that
cannot do what its label says.

`options` are labels, in full. A bare "A" or "2" is not an answer target, it is
a lookup the user has to run against a terminal they may not be looking at.

`children` counts open subagents only, and feeds the rule that a session with
live children never reads as complete. Work a runtime starts without announcing
it, a backgrounded shell command for instance, cannot be counted, and a count
that quietly undercounts is worse than none. See
[ARCHITECTURE.md](ARCHITECTURE.md#the-child-ledger).

`error` is a pair rather than a string because a tile has to say something
short and true about a failure. `kind` is the runtime's own classification
where it has one, `message` is free text for the detail panel.

Liveness is the daemon's job, not the adapter's. The daemon brackets each
operation from the updates it is sent and times the session out from the last
one, so an adapter has to report the end of an operation as well as its start,
including the ends that are not a result: a denial, an interrupt, a decision
handed back. A bracket that is never closed is the same defect as a wrong
colour, arriving more slowly. The rule the daemon applies is in
[ARCHITECTURE.md](ARCHITECTURE.md#liveness-by-open-operation).

`ask` matters. It is the safe answer when Deckhand cannot get a human decision
in time, and it is what makes failing closed possible without denying work the
user actually wanted. See [SECURITY_MODEL.md](SECURITY_MODEL.md).

## Interface

```ts
interface Adapter {
  readonly name: string;

  // The most this adapter can ever offer, across every host it supports.
  // It is a ceiling, not a promise about any one session: read
  // `SessionInfo.capabilities` to decide whether a control is live.
  readonly capabilities: Record<Capability, Confidence | false>;

  start(ctx: AdapterContext): Promise<void>;
  stop(): Promise<void>;

  listSessions(): Promise<SessionInfo[]>;

  // Optional, gated on the matching capability.
  focusSession?(id: SessionId): Promise<void>;
  sendPrompt?(id: SessionId, text: string): Promise<void>;
  interrupt?(id: SessionId): Promise<void>;
  setOption?(id: SessionId, key: string, value: unknown): Promise<void>;
  decidePermission?(requestId: string, d: PermissionDecision): Promise<void>;

  // Gated on `answer_question`, which no adapter can declare yet. The option
  // is passed as its label, the same text the user clicked.
  answerQuestion?(id: SessionId, option: string): Promise<void>;
}

interface AdapterContext {
  onSessionUpdate(u: SessionUpdate): void;
  onPermissionRequest(r: DeckhandPermissionRequest): void;
  onSessionGone(id: SessionId): void;
  log(level: "debug" | "info" | "warn" | "error", msg: string): void;
}
```

Adapters push. The daemon does not poll them. An adapter that can only poll its
runtime does that polling internally and still pushes the result, so that
polling never becomes the daemon's problem.

## Rules every adapter must follow

1. **Never invent a state.** If you do not know, report `unknown`. This is the
   most important rule in the contract. A status board is worth having only if
   its colours can be trusted, and one confident-looking wrong colour costs more
   than a hundred honest grey ones. It binds hardest at cold start, where an
   enumeration hands you sessions whose history you never saw: map only what
   the runtime states, and map an unrecognised or absent status to `unknown`.
   Never to `idle`. Idle is the one guess that looks like knowledge.
2. **Report the observation time, not the delivery time.** State can arrive out
   of order. The daemon uses `at` to resolve that.
3. **Answer every permission request before it expires.** If you cannot get an
   answer, answer `ask` and let the runtime prompt the user normally.
4. **Be idempotent.** The same update delivered twice must not change anything.
5. **Degrade to observation.** If the acting side breaks, keep reporting status.
   A read-only Deckhand is still useful. A crashed one is not.
6. **Never block the runtime you are observing.** An adapter's failure must not
   stop the user's actual work.
7. **Declare confidence honestly.** Marking something `documented` when it is
   scraped from an internal is the kind of shortcut that makes a tool untrustworthy.
8. **Report a lifecycle change only when the lifecycle changed.** Runtimes
   reuse start and end events for things that are neither, a compaction that
   arrives as a session start being the common case. Read the reason before
   reporting `idle` or `ended`, and report nothing at all for a reason you do
   not recognise. This rule exists because both of the wrong colours found in
   review so far were lifecycle events taken at face value.

## What is deliberately not in the contract

- **Transcript content.** Adapters report state and small details, not
  conversation history. Rendering a transcript is a different job with different
  privacy properties, and folding it in here would push every adapter to parse
  formats their runtime does not promise to keep stable.
- **Authentication and billing.** Not Deckhand's business.
- **Starting sessions**, except in hosted mode, which is a separate and more
  demanding interface not specified until Phase 4.

## Versioning

`capabilities` is additive. Removing a capability or changing the meaning of a
`SessionState` is a breaking change and bumps the protocol version. The daemon
refuses to load an adapter that declares a protocol version it does not know,
rather than loading it and hoping.

## Writing an adapter

There is nothing to write against yet. When there is, the shape of the work is:
identify how the runtime reveals state, map it onto the seven states above, find
out whether permission decisions can be intercepted, and be honest in the
capability table about which of those rest on documented interfaces. If you are
considering one, open an
[adapter request](https://github.com/owenpkent/deckhand/issues/new?template=adapter_request.yml)
first so the contract can be checked against a second runtime before it
calcifies around the first.
