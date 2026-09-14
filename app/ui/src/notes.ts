// Row notes are keyed by session id, not row index (PR review:
// identity). The two old IPC calls (select_tile, reveal_session) each
// resolved the same numeric row index at a different moment; a row
// disappearing between them could land a note -- or a selection, or a
// reveal -- on the wrong session. Indices still shift as sessions bind
// and unbind (registry.rs), but a note now stays pinned to the session
// it was raised for no matter how the rows above it move, right up
// until that session itself is gone.
//
// Pure and DOM-free, like format.ts, so the bookkeeping is testable
// without a browser; main.ts owns the actual Map and setTimeout calls.

// Takes the minimal per-tile shape rather than the full TileSnapshot
// (the same choice format.ts's unknownCount makes), so it stays
// DOM-free and easy to test.

/** Every session id currently present in a snapshot's tiles. A tile
 * with no session (the defensive row-empty case) contributes nothing. */
export function presentSessionIds(tiles: readonly { session: { id: string } | null }[]): Set<string> {
  return new Set(tiles.flatMap((t) => (t.session ? [t.session.id] : [])));
}

/** The ids in `noteIds` that name a session no longer in `present`, in
 * the order seen. Once a session is gone its note has nowhere left to
 * attach: dropping it here is what stops a note ever showing up on a
 * different row later, whether through reuse or through simply never
 * being cleaned up. */
export function staleNoteIds(noteIds: Iterable<string>, present: ReadonlySet<string>): string[] {
  const stale: string[] = [];
  for (const id of noteIds) {
    if (!present.has(id)) stale.push(id);
  }
  return stale;
}
