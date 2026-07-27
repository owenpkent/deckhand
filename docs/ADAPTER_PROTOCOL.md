# Adapter protocol

Status: **proposed**, version `0`. Expect it to change once a second adapter
exists, because a contract with one implementation is a guess.

An adapter connects Deckhand to one agent runtime. The daemon knows only this
contract. Everything specific to Claude Code lives behind it, documented in
[CLAUDE_CODE_ADAPTER.md](CLAUDE_CODE_ADAPTER.md).

## Capabilities

An adapter declares what it can do. The surface reads these declarations and
disables controls it cannot drive, so a missing capability is a greyed-out
button with a tooltip explaining why, never a button that silently does nothing.

| Capability | Meaning | Required |
| --- | --- | --- |
| `observe_status` | Report session state changes | Yes |
| `list_sessions` | Enumerate sessions available to bind | Yes |
| `focus_session` | Raise the session's window | No |
| `decide_permission` | Answer a pending permission request | No |
| `send_prompt` | Put a prompt into a session | No |
| `interrupt` | Stop the current turn | No |
| `set_option` | Change a session option, for example the model | No |

Only the first two are required. An adapter that can do nothing but report
status is still useful: the status board is most of the value.

Each capability also carries a **confidence**, because "supported" and
"supported by a documented interface" are different claims:

| Confidence | Meaning |
| --- | --- |
| `documented` | Built on a public interface the runtime commits to |
| `internal` | Works, but depends on something undocumented that may break |
| `synthetic` | Simulated from outside, for example by sending keystrokes at a window |

The surface must show `synthetic` differently from `documented`. A user pressing
Approve deserves to know whether that is a real API call or a best guess.

## Types

Sketched in TypeScript for readability. The real boundary is Rust.

```ts
type SessionId = string;

type SessionState =
  | "idle" | "thinking" | "needs_input"
  | "complete" | "error" | "ended" | "unknown";

interface SessionInfo {
  id: SessionId;
  adapter: string;          // "claude-code"
  label: string;            // human-facing, usually the project directory
  cwd: string;
  mode: "attached" | "hosted";
  model?: string;
  startedAt: string;        // ISO 8601
}

interface SessionUpdate {
  id: SessionId;
  state: SessionState;
  at: string;               // ISO 8601, when the adapter observed it
  detail?: {
    tool?: string;          // what is running, for the tile subtitle
    question?: string;      // what is being asked, when state is needs_input
    contextUsedPct?: number;
    costUsd?: number;
    error?: string;
  };
}

interface PermissionRequest {
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

`ask` matters. It is the safe answer when Deckhand cannot get a human decision
in time, and it is what makes failing closed possible without denying work the
user actually wanted. See [SECURITY_MODEL.md](SECURITY_MODEL.md).

## Interface

```ts
interface Adapter {
  readonly name: string;
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
}

interface AdapterContext {
  onSessionUpdate(u: SessionUpdate): void;
  onPermissionRequest(r: PermissionRequest): void;
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
   than a hundred honest grey ones.
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
