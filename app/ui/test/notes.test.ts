// Pins the pure helpers behind row notes (src/notes.ts): notes are keyed
// by session id, not row index, so a note raised for one session can
// never end up attached to whatever row a different session now
// occupies (PR review: identity). These tests exercise that lifecycle
// through presentSessionIds/staleNoteIds directly, since main.ts itself
// touches the DOM and cannot be imported under node:test.

import assert from "node:assert/strict";
import test from "node:test";

import { presentSessionIds, staleNoteIds } from "../src/notes.js";

function tile(id: string | null) {
  return { session: id ? { id } : null };
}

// ---- presentSessionIds -------------------------------------------------

test("presentSessionIds collects every bound session's id", () => {
  const ids = presentSessionIds([tile("a"), tile("b"), tile("c")]);
  assert.equal([...ids].sort().join(","), "a,b,c");
});

test("presentSessionIds skips a tile with no session", () => {
  // Defensive only: every bound row should resolve to a session, but a
  // stray null must not poison the set with a bogus id.
  const ids = presentSessionIds([tile("a"), tile(null)]);
  assert.equal([...ids].join(","), "a");
});

test("presentSessionIds on no tiles is empty", () => {
  assert.equal(presentSessionIds([]).size, 0);
});

// ---- staleNoteIds -------------------------------------------------------

test("staleNoteIds returns ids no longer present", () => {
  const present = presentSessionIds([tile("b"), tile("c")]);
  assert.equal(staleNoteIds(["a", "b"], present).join(","), "a");
});

test("staleNoteIds returns nothing when every note's session is present", () => {
  const present = presentSessionIds([tile("a"), tile("b")]);
  assert.equal(staleNoteIds(["a", "b"], present).length, 0);
});

test("staleNoteIds preserves the order notes were seen in", () => {
  const present = new Set<string>();
  assert.equal(staleNoteIds(["z", "a", "m"], present).join(","), "z,a,m");
});

// ---- The lifecycle these two functions exist for -----------------------
//
// A row's session id is captured in the click closure (main.ts), so a
// reply that arrives after the rows have reshuffled still targets the
// right note. These tests pin the two PR-review regression scenarios at
// the level main.ts's pruneRowNotes actually operates on: a notes map
// and a snapshot's present ids.

test("a delayed miss for b survives an earlier row (a) ending first", () => {
  const notes = new Map<string, string>();
  // a ends; b is still bound when the miss for b finally arrives.
  const afterAEnded = [tile("b")];
  notes.set("b", "No window matched");
  for (const id of staleNoteIds(notes.keys(), presentSessionIds(afterAEnded))) {
    notes.delete(id);
  }
  assert.equal(notes.get("b"), "No window matched", "b's note must survive a's removal");
});

test("once b itself ends, no surviving row inherits its note", () => {
  const notes = new Map<string, string>();
  notes.set("b", "No window matched");
  // b has now ended too; only c remains bound.
  const afterBEnded = [tile("c")];
  for (const id of staleNoteIds(notes.keys(), presentSessionIds(afterBEnded))) {
    notes.delete(id);
  }
  assert.equal(notes.size, 0, "b's note must be dropped, not reattached to c");
  assert.equal(notes.get("c"), undefined, "c never had a note and must not gain one");
});
