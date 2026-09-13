// Compile-time guard: SessionSnap["state"] must stay exactly the seven
// states ADR-008 freezes (docs/DECISIONS.md#adr-008). Assigning each
// literal proves none is missing; the @ts-expect-error line proves the
// union has not been silently widened to accept an eighth state. This
// file's job is done at `tsc -p tsconfig.test.json` time, not at runtime,
// but node:test needs at least one assertion to count the file.

import assert from "node:assert/strict";
import test from "node:test";

import type { SessionSnap } from "../src/types.js";

type State = SessionSnap["state"];

const idleState: State = "idle";
const thinkingState: State = "thinking";
const needsInputState: State = "needs_input";
const completeState: State = "complete";
const errorState: State = "error";
const endedState: State = "ended";
const unknownState: State = "unknown";

// @ts-expect-error "paused" is not one of the seven frozen states. If this
// stops erroring, the union has been widened by accident and the build
// should fail on the unused-directive error instead of passing quietly.
const pausedState: State = "paused";

test("the seven frozen states are all distinct", () => {
  const states = [
    idleState,
    thinkingState,
    needsInputState,
    completeState,
    errorState,
    endedState,
    unknownState,
  ];
  assert.equal(new Set(states).size, 7);
  // pausedState only exists to carry the @ts-expect-error above; touch it
  // so it is not flagged as an unused local under stricter lint configs.
  assert.equal(typeof pausedState, "string");
});
