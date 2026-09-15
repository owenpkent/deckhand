// The session registry: every session the daemon knows about, the
// ordered list of sessions the surface shows, and the snapshot the
// surface renders. Bindings are by session id, which survives restarts
// (docs/ARCHITECTURE.md#persistence).
//
// The list auto-binds: any session heard from, by a hook event or by
// enumeration, appends to it the first time it is seen, in the order it
// was seen. There is no picker and no fixed slot count.
//
// ADR-035 split who is *listed* from who is *coloured*. A row leaves the
// list for one of three reasons: its `SessionEnd` arrives; its process
// handle (`liveness.rs`, held in `watches` below) reports the process
// gone, which unbinds it exactly like a `SessionEnd` would even though
// none arrived; or, only for a session with no handle at all, a
// successful enumeration omits it for `ENUM_GRACE_MS` with no recent
// hook (`prune_missing`). A session with a live handle survives any
// number of scans that omit it: the handle is the more trustworthy
// signal, and prune_missing checks it before dropping anything. What
// colour a listed row shows is a separate question, answered by
// `state.rs` (`apply_hook` and, now, `apply_scan_state`) and by
// `Session::tick`, not by list membership. ADR-036 adds one exception to
// the hook-always-wins rule inside `apply_scan_state`: two consecutive
// scans that contradict a hook-set colour, with no hook event landing
// between them, recolour the session anyway.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use crate::host;
use crate::liveness;
use crate::state::{Session, SessionState};
use crate::supersede;

/// A session that vanished from a successful enumeration less than this
/// long ago is kept rather than pruned: enumeration and the hook stream
/// are two independent channels, and a live hook is the more trustworthy
/// of the two when they briefly disagree.
pub const ENUM_GRACE_MS: i64 = 60_000;

#[derive(Default)]
pub struct Registry {
    pub sessions: HashMap<String, Session>,
    /// The ordered, unbounded list of bound session ids. Index into this
    /// is the row index the surface renders, but it is presentation
    /// only: a row's click carries the session id, never this index, so
    /// a row disappearing between two IPC calls can never make one land
    /// on the wrong session (`begin_activation`, PR review: identity).
    pub bindings: Vec<String>,
    pub selected: Option<usize>,
    /// The header's grey toggle. Owned here so `snapshot` can report it
    /// and so a resize can be sized off the visible row count; persisted
    /// by persist.rs, not here.
    pub hide_unknown: bool,
    /// One process handle per session that has ever had a pid, keyed by
    /// session id (ADR-035, `liveness.rs`). Not `Session` state and not
    /// part of `Snapshot`: it is an OS resource, not a fact about the
    /// session worth showing, and `Registry` itself is never serialised.
    watches: HashMap<String, liveness::Watch>,
}

/// A manual `Debug` impl because `liveness::Watch` (`Box<dyn Liveness>`)
/// does not implement it and need not: the handles themselves carry
/// nothing worth printing beyond which sessions currently have one.
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("sessions", &self.sessions)
            .field("bindings", &self.bindings)
            .field("selected", &self.selected)
            .field("hide_unknown", &self.hide_unknown)
            .field("watches", &self.watches.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSnapshot {
    pub index: usize,
    pub selected: bool,
    pub session: Option<Session>,
}

/// Everything a reveal needs, cloned out of a `Session` while the
/// registry lock is held so no borrow of it ever has to cross into the
/// (potentially slow) reveal worker or an `await` (PR review: no
/// registry guard or borrowed `Session` crosses into that work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealRequest {
    pub session_id: String,
    pub label: String,
    pub cwd: Option<String>,
    pub dir: Option<String>,
    pub pid: Option<u32>,
}

/// What a click's `activate_session` IPC call resolves to. A stale
/// click, one whose session id is no longer bound by the time it
/// reaches the daemon, is an explicit `Miss`: no substitution to
/// whatever now occupies that row, no selection. Otherwise the id is
/// selected right here (acknowledging its unread complete) and `Go`
/// carries the owned request the caller hands to the reveal worker
/// after releasing the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    Miss,
    Go(RevealRequest),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tiles: Vec<TileSnapshot>,
    pub now_ms: i64,
    /// Mirrors `Registry::hide_unknown`. Tiles are never filtered out of
    /// this list on account of it; the surface decides what to draw.
    pub hide_unknown: bool,
}

impl Registry {
    /// Apply a raw hook payload. Returns true when the surface should be
    /// repainted. Events without a session id cannot be attributed and
    /// are dropped here (they were still captured upstream by the shim's
    /// caller for the payload spike).
    pub fn apply_hook(&mut self, payload: &Value, now_ms: i64) -> bool {
        let Some(id) = payload.get("session_id").and_then(Value::as_str) else {
            return false;
        };
        let session = self
            .sessions
            .entry(id.to_string())
            .or_insert_with(|| Session::new(id.to_string(), now_ms));
        let changed = session.apply_hook(payload, now_ms);
        let ended = session.state == SessionState::Ended;
        // A session heard from for the first time joins the list, even
        // on an event that left its own state unchanged: liveness alone
        // is enough to prove it exists. An ended session leaves the list
        // outright rather than lingering as a dead row, and its process
        // handle, if it had one, is dropped along with it (ADR-035): a
        // `SessionEnd` is as authoritative an ending as a handle
        // reporting the process gone, and holding the handle open past
        // it serves nothing.
        let list_changed = if ended {
            self.drop_watch(id);
            self.unbind_id(id)
        } else {
            self.auto_bind(id)
        };
        changed || list_changed
    }

    /// Register a session found by enumeration (`claude agents --json`).
    /// ADR-035: state is no longer left unknown by rule. A `status` key
    /// has been observed since ADR-024 and colours the session
    /// (`Session::apply_scan_state`) precisely when no hook has already
    /// coloured it; a hook still always wins, except for ADR-036's
    /// tie-break, which lets two consecutive scans that disagree with a
    /// hook-set colour, and no hook event landing between them, override
    /// it anyway. Also auto-binds: enumeration is the only channel that
    /// ever sees a session in another repo, one with no hook wired up at
    /// all.
    pub fn register_enumerated(
        &mut self,
        id: &str,
        name: Option<&str>,
        cwd: Option<&str>,
        pid: Option<u32>,
        status: Option<&str>,
        now_ms: i64,
    ) -> bool {
        let mut changed = false;
        let mut pid_changed = false;
        if let Some(existing) = self.sessions.get_mut(id) {
            // A session the hooks already saw end stays off the list and
            // untouched by enumeration entirely, checked before anything
            // below runs: not rebound, not given a fresh pid (the OS is
            // free to reuse an ended session's pid for an unrelated
            // process, so a scan naming it is not naming this session's
            // process any more), not state, not cwd or label. Only a
            // session-start event (state.rs) revives it.
            if existing.state == SessionState::Ended {
                return false;
            }
            existing.note_seen(status, now_ms);
            changed |= existing.apply_scan_state(status, now_ms);
            // A pid is never in a hook payload and Reveal wants one.
            // Unlike a blank field, a pid already present is replaced
            // rather than left alone when the scan reports a different
            // one: a resumed session runs under a new OS process, and
            // holding onto the stale pid would point Reveal (and the
            // liveness watch below) at whatever that pid now names. A
            // scan that saw no pid this time (`pid: None`) leaves
            // whatever is recorded alone rather than clearing it.
            if let Some(new_pid) = pid {
                if existing.pid != Some(new_pid) {
                    existing.pid = Some(new_pid);
                    changed = true;
                    pid_changed = true;
                }
            }
            // Unlike pid, cwd is corrected rather than only filled in:
            // enumeration reports the session's own cwd, so it is the
            // one channel that can repair a value a subagent payload
            // latched onto the session before this fix (state.rs
            // apply_hook).
            //
            // Label precedence is the same regardless of which channel
            // saw this session first (2026-09-15's inconsistent-label
            // report: a hook-first session's label came only from cwd,
            // a scan-first session's from the scan's own `name`, and the
            // two never agreed). A derived label -- `label_is_derived`,
            // set wherever a label is ever assigned from cwd, not
            // compared by string -- stays open to being replaced by a
            // real `name`; a real one, once seen, never is again.
            if let Some(cwd) = cwd {
                if existing.cwd.as_deref() != Some(cwd) {
                    existing.cwd = Some(cwd.to_string());
                    changed = true;
                }
                if existing.label.is_empty() || existing.label_is_derived {
                    let derived = crate::state::dir_name(cwd);
                    if existing.label != derived {
                        existing.label = derived;
                        changed = true;
                    }
                    existing.label_is_derived = true;
                }
            }
            // A real name is accepted for its source, not its text: a
            // session in `C:/dev/deckhand` genuinely named `deckhand`
            // clears `label_is_derived` even though the string does not
            // move, because the flag is what protects the label from
            // the next cwd-only row, and a scan-first session with the
            // same name is already protected that way. That flag-only
            // transition counts as a change so it reaches
            // `bindings.json` (persist.rs) and survives a restart.
            if let Some(n) = name {
                if !n.is_empty() && (existing.label.is_empty() || existing.label_is_derived) {
                    if existing.label != n {
                        existing.label = n.to_string();
                    }
                    existing.label_is_derived = false;
                    changed = true;
                }
            }
        } else {
            let mut s = Session::new(id.to_string(), now_ms);
            s.pid = pid;
            if let Some(cwd) = cwd {
                s.cwd = Some(cwd.to_string());
                s.label = crate::state::dir_name(cwd);
                s.label_is_derived = true;
            }
            if let Some(n) = name {
                if !n.is_empty() {
                    s.label = n.to_string();
                    s.label_is_derived = false;
                }
            }
            s.note_seen(status, now_ms);
            s.apply_scan_state(status, now_ms);
            self.sessions.insert(id.to_string(), s);
            changed = true;
            pid_changed = pid.is_some();
        }
        // A pid recorded for the first time or replaced gets a fresh
        // liveness watch (ADR-035): the old handle, if any, is dropped
        // first since it names a pid this session no longer runs under,
        // then a new one is opened when possible. `liveness::open`
        // returning `None` (the platform stub, or a pid that could not
        // be opened) simply leaves the session with no watch, the same
        // as before any pid was ever seen.
        if pid_changed {
            self.drop_watch(id);
            if let Some(p) = pid {
                if let Some(watch) = liveness::open(p) {
                    self.set_watch(id, watch);
                }
                // Host and immediate parent pid (supersede.rs) are
                // resolved here, once per pid change, and stored rather
                // than re-walked on every tick: the process tree behind
                // a pid does not change out from under it.
                let (host, parent_pid) = host::resolve(p);
                if let Some(s) = self.sessions.get_mut(id) {
                    s.host = Some(host);
                    s.parent_pid = parent_pid;
                }
            }
        }
        changed |= self.auto_bind(id);
        changed
    }

    /// Record the scan's own `startedAt` for this session
    /// (`supersede::superseded`'s ordering), called by
    /// `enumerate::register` right after `register_enumerated` for the
    /// same row. Reports whether the stored value actually changed,
    /// and `enumerate::register` folds that into its own change flag:
    /// a `startedAt` arriving or moving can flip which rows
    /// `superseded_ids` hides with no other field changing (two idle
    /// rows tied on first-seen, then a scan naming distinct start
    /// times), and the scan loop only repaints and resizes when
    /// registration says something changed. An identical follow-up
    /// scan is a no-op here as everywhere else. A session
    /// `register_enumerated` left untouched (already `Ended`) is left
    /// alone here too, and a scan that reports no `startedAt` this time
    /// (`None`) leaves whatever is already recorded alone, the same as
    /// a missing pid does.
    pub fn note_started_at(&mut self, id: &str, started_at_ms: Option<i64>) -> bool {
        let Some(ms) = started_at_ms else {
            return false;
        };
        let Some(s) = self.sessions.get_mut(id) else {
            return false;
        };
        if s.state == SessionState::Ended || s.started_at_ms == Some(ms) {
            return false;
        }
        s.started_at_ms = Some(ms);
        true
    }

    /// The facts `supersede::superseded` needs about every currently
    /// bound session, gathered fresh off the registry rather than kept
    /// as state of its own (`superseded_ids`'s own doc comment has the
    /// reasoning for why that is safe).
    fn supersession_candidates(&self) -> Vec<supersede::Candidate<'_>> {
        self.bindings
            .iter()
            .filter_map(|id| self.sessions.get(id))
            .map(|s| supersede::Candidate {
                id: &s.id,
                pid: s.pid,
                host: s.host,
                parent_pid: s.parent_pid,
                cwd: s.cwd.as_deref(),
                state: s.state,
                started_at_ms: s.started_at_ms,
                first_seen_ms: s.first_seen_ms,
            })
            .collect()
    }

    /// Every bound session id a newer one in the same VS Code window and
    /// folder currently makes safe to hide (`supersede::superseded`;
    /// the 2026-09-15 four-session report). Pure and recomputed on every
    /// call rather than cached: `snapshot` calls it to build the tiles
    /// list the surface actually sees, and `main.rs::visible_rows` calls
    /// it too, to size the window off the same set. Nothing here unbinds
    /// anything -- a hidden session keeps its place in `bindings` so it
    /// resumes exactly where it was the moment it stops qualifying, with
    /// no bookkeeping of its own to undo.
    pub fn superseded_ids(&self) -> HashSet<String> {
        supersede::superseded(&self.supersession_candidates())
    }

    /// Give a session a liveness watch, replacing whatever it had.
    pub fn set_watch(&mut self, id: &str, watch: liveness::Watch) {
        self.watches.insert(id.to_string(), watch);
    }

    /// Drop a session's liveness watch, if it has one. A no-op otherwise.
    fn drop_watch(&mut self, id: &str) {
        self.watches.remove(id);
    }

    /// Whether the registry holds a watch for `id` whose process has not
    /// reported exiting. False for a session with no watch at all, the
    /// same as one whose watch has fired: neither is proof of life.
    fn is_alive(&self, id: &str) -> bool {
        self.watches.get(id).map(|w| !w.exited()).unwrap_or(false)
    }

    /// Insert a placeholder for a session known only by id and label,
    /// in unknown state, without touching one that already exists. Used
    /// to restore a cold-start binding from disk.
    pub fn ensure_session(&mut self, id: &str, label: &str, now_ms: i64) {
        self.sessions.entry(id.to_string()).or_insert_with(|| {
            let mut s = Session::new(id.to_string(), now_ms);
            s.label = label.to_string();
            s
        });
    }

    pub fn is_bound(&self, id: &str) -> bool {
        self.bindings.iter().any(|b| b == id)
    }

    /// Append `id` to the list if it is not already there. Returns true
    /// when the list changed.
    fn auto_bind(&mut self, id: &str) -> bool {
        if self.is_bound(id) {
            return false;
        }
        self.bindings.push(id.to_string());
        true
    }

    /// Drop `id` from the list without touching `self.sessions`: the
    /// session's own record (and history) is kept, only its row goes
    /// away. Returns true when the list changed.
    fn unbind_id(&mut self, id: &str) -> bool {
        let Some(pos) = self.bindings.iter().position(|b| b == id) else {
            return false;
        };
        self.bindings.remove(pos);
        // Selection is by index, so keep it on the same session when a
        // row above it goes, and clear it when the selected row itself
        // is the one leaving.
        self.selected = match self.selected {
            Some(sel) if sel == pos => None,
            Some(sel) if sel > pos => Some(sel - 1),
            other => other,
        };
        true
    }

    /// Drop bound sessions absent from a successful enumeration, unless
    /// they received a hook event within `ENUM_GRACE_MS` or a held
    /// process handle says the session is still alive (ADR-035): a scan
    /// that omits a session it should have listed is exactly the case a
    /// handle exists to catch. Must only be called after an enumeration
    /// run that actually succeeded: a failed run (claude missing,
    /// unparseable output) carries no information about who is still
    /// alive and must prune nothing.
    pub fn prune_missing(&mut self, present: &HashSet<String>, now_ms: i64) -> bool {
        let stale: Vec<String> = self
            .bindings
            .iter()
            .filter(|id| !present.contains(id.as_str()))
            .filter(|id| {
                let recent_hook = self
                    .sessions
                    .get(id.as_str())
                    .map(|s| now_ms.saturating_sub(s.last_event_at_ms) < ENUM_GRACE_MS)
                    .unwrap_or(false);
                !recent_hook
            })
            .filter(|id| !self.is_alive(id))
            .cloned()
            .collect();
        let mut changed = false;
        for id in stale {
            changed |= self.unbind_id(&id);
        }
        changed
    }

    fn select(&mut self, index: usize, now_ms: i64) -> bool {
        if index >= self.bindings.len() {
            return false;
        }
        let mut changed = self.selected != Some(index);
        self.selected = Some(index);
        let id = self.bindings[index].clone();
        if let Some(s) = self.sessions.get_mut(&id) {
            changed |= s.on_selected(now_ms);
        }
        changed
    }

    /// Resolve a row click's session id: `Miss` when it is no longer
    /// bound (a row above it may have ended between the click and this
    /// call; there is no row left to fall back to, so nothing is
    /// selected), otherwise select it by its *current* index -- never a
    /// stale one the caller might be holding -- and clone what the
    /// reveal worker needs into an owned `RevealRequest`. Pure registry
    /// state in, `Activation` out: no lock, no Tauri type, so this is
    /// tested without a webview (PR review: identity + blocking).
    pub fn begin_activation(&mut self, id: &str, now_ms: i64) -> Activation {
        let Some(pos) = self.bindings.iter().position(|b| b == id) else {
            return Activation::Miss;
        };
        self.select(pos, now_ms);
        let session = self.sessions.get(id).expect("just resolved from bindings");
        Activation::Go(RevealRequest {
            session_id: session.id.clone(),
            label: session.label.clone(),
            cwd: session.cwd.clone(),
            dir: session.cwd.as_deref().map(crate::state::dir_name),
            pid: session.pid,
        })
    }

    /// Runs every 2 seconds under the registry lock (main.rs's tick
    /// thread); `WaitForSingleObject(h, 0)` per watch is microseconds,
    /// so that cadence is fine even with many watches open. For each
    /// bound session (ADR-035), a watch that reports the process exited
    /// ends it outright (`process_exited`, then the watch is dropped
    /// and the row unbound, exactly like a `SessionEnd` (only a fresh
    /// `SessionStart` revives it), and every other session ticks
    /// against `T_unknown` with `alive` reflecting whether it still has
    /// a live watch. Unbound sessions are not visited: nothing on screen
    /// depends on their colour, and a session pruned from the list but
    /// still alive is caught by `prune_missing` instead of here.
    pub fn tick(&mut self, now_ms: i64) -> bool {
        let mut changed = false;
        let ids: Vec<String> = self.bindings.clone();
        for id in ids {
            let exited = self.watches.get(&id).map(|w| w.exited()).unwrap_or(false);
            if exited {
                self.drop_watch(&id);
                if let Some(s) = self.sessions.get_mut(&id) {
                    changed |= s.process_exited(now_ms);
                }
                changed |= self.unbind_id(&id);
                continue;
            }
            let alive = self.watches.contains_key(&id);
            if let Some(s) = self.sessions.get_mut(&id) {
                changed |= s.tick(now_ms, alive);
            }
        }
        changed
    }

    /// A superseded session (`superseded_ids`) is left out of the tiles
    /// list entirely rather than unbound: its place in `bindings`, and
    /// `self.selected`'s index into it, are both untouched, so if it
    /// stops qualifying on a later call it reappears, selected or not,
    /// exactly as it was. `selected` below is therefore compared against
    /// the *original* binding index (`bound_index`), not the filtered
    /// row's own position, and `index` renders as the sequential
    /// position in the filtered list actually shown -- presentation
    /// only, as everywhere else it is used.
    pub fn snapshot(&self, now_ms: i64) -> Snapshot {
        let hidden = self.superseded_ids();
        Snapshot {
            tiles: self
                .bindings
                .iter()
                .enumerate()
                .filter(|(_, id)| !hidden.contains(id.as_str()))
                .enumerate()
                .map(|(index, (bound_index, id))| TileSnapshot {
                    index,
                    selected: self.selected == Some(bound_index),
                    session: self.sessions.get(id).cloned(),
                })
                .collect(),
            now_ms,
            hide_unknown: self.hide_unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_event_appends_to_an_empty_list() {
        let mut r = Registry::default();
        r.apply_hook(
            &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1", "cwd": "C:/dev/a"}),
            1,
        );
        assert_eq!(r.bindings, vec!["s1".to_string()]);
    }

    #[test]
    fn sessions_bind_in_the_order_they_are_first_heard() {
        let mut r = Registry::default();
        for i in 0..8 {
            r.apply_hook(
                &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": format!("s{i}")}),
                1,
            );
        }
        let expected: Vec<String> = (0..8).map(|i| format!("s{i}")).collect();
        assert_eq!(r.bindings, expected, "there is no fixed slot count: an eighth session still gets a row");
    }

    #[test]
    fn an_event_that_does_not_change_state_still_binds_a_new_session() {
        // SessionEnd with reason "resume" is a no-op for the state
        // machine (see state.rs), but the session must still appear.
        let mut r = Registry::default();
        let changed = r.apply_hook(
            &json!({"hook_event_name": "SessionEnd", "reason": "resume", "session_id": "s1"}),
            1,
        );
        assert!(changed, "binding a brand new session is itself a repaint-worthy change");
        assert!(r.is_bound("s1"));
    }

    #[test]
    fn a_session_that_ends_leaves_the_list_but_keeps_its_record() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        assert!(r.is_bound("s1"));
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 2);
        assert!(!r.is_bound("s1"), "an ended session must leave the list");
        assert!(r.sessions.contains_key("s1"), "the session record itself must survive");
    }

    #[test]
    fn removing_a_row_above_the_selection_keeps_the_same_session_selected() {
        let mut r = Registry::default();
        for id in ["s1", "s2", "s3"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id}), 1);
        }
        assert!(r.select(2, 2));
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 3);
        assert_eq!(r.bindings, vec!["s2".to_string(), "s3".to_string()]);
        assert_eq!(r.selected, Some(1), "selection follows s3 to its new index");
    }

    #[test]
    fn removing_the_selected_row_clears_the_selection() {
        let mut r = Registry::default();
        for id in ["s1", "s2"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id}), 1);
        }
        assert!(r.select(1, 2));
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s2"}), 3);
        assert_eq!(r.selected, None);
        assert!(!r.select(1, 4), "the old index is out of range and must not select anything");
    }

    // ---- begin_activation: identity survives a row moving underneath --
    //
    // PR review (identity): the two old IPC calls (select_tile,
    // reveal_session) each resolved the same numeric index at a
    // different moment; a row vanishing between them could select or
    // reveal the wrong session. begin_activation takes the id instead,
    // so these pin that a row's disappearance around the click can only
    // ever affect that row's own id, never a neighbour's.

    #[test]
    fn activating_a_session_by_id_survives_a_removal_above_it() {
        let mut r = Registry::default();
        for id in ["a", "b", "c"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id}), 1);
        }
        // Give b an unread complete so activating it has something to
        // acknowledge.
        r.apply_hook(&json!({"hook_event_name": "Stop", "session_id": "b"}), 2);
        assert_eq!(r.sessions["b"].state, SessionState::Complete);

        // The row above b disappears before the click resolves.
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "a"}), 3);
        assert_eq!(r.bindings, vec!["b".to_string(), "c".to_string()], "b is now row 0, not row 1");

        let activation = r.begin_activation("b", 4);
        let Activation::Go(request) = activation else {
            panic!("b is still bound; this must not be a miss");
        };
        assert_eq!(request.session_id, "b");
        assert_eq!(r.selected, Some(0), "b's current index is selected, not a stale one the caller might hold");
        assert_eq!(r.sessions["b"].state, SessionState::Idle, "activating b acknowledges its own unread complete");
        assert_eq!(r.sessions["c"].unread_since_ms, None, "c was never touched");
    }

    #[test]
    fn a_request_built_by_begin_activation_is_unaffected_by_a_later_removal() {
        let mut r = Registry::default();
        for id in ["a", "b"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id, "cwd": format!("C:/dev/{id}")}), 1);
        }
        let Activation::Go(request) = r.begin_activation("b", 2) else {
            panic!("b is bound")
        };
        // a disappears after the request is built; the worker that
        // eventually receives `request` never touches the registry, so
        // this must not be able to change it (PR review: no borrowed
        // Session crosses into that work; this is the owned-clone half
        // of that guarantee).
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "a"}), 3);
        assert_eq!(request.session_id, "b");
        assert_eq!(request.dir.as_deref(), Some("b"));
    }

    #[test]
    fn activating_a_session_removed_before_the_click_arrives_is_a_miss() {
        let mut r = Registry::default();
        for id in ["a", "b", "c"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id}), 1);
        }
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "b"}), 2);
        assert_eq!(r.bindings, vec!["a".to_string(), "c".to_string()]);

        let activation = r.begin_activation("b", 3);
        assert_eq!(activation, Activation::Miss);
        assert_eq!(r.selected, None, "a miss must not select anything, including whatever now sits in b's old row");
    }

    #[test]
    fn a_lagging_enumeration_does_not_rebind_an_ended_session_or_its_pid() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 2);
        assert_eq!(r.sessions["s1"].pid, None, "SessionEnd already cleared it");
        // A pid the scan reports here could already belong to an
        // unrelated process; an ended session must be left untouched,
        // not merely unbound.
        assert!(!r.register_enumerated("s1", None, None, Some(7), None, 3));
        assert!(!r.is_bound("s1"), "hooks saw it end; the enumeration is stale");
        assert_eq!(r.sessions["s1"].pid, None, "a stale scan must not repopulate the pid either");
    }

    #[test]
    fn a_straggler_event_after_session_end_stays_ended_and_off_the_list() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 2);
        assert!(!r.is_bound("s1"));

        // A Stop delivered late (out of order, or racing SessionEnd
        // itself) used to flip the session back to Complete, and
        // Registry::apply_hook reads state after applying the event, so
        // it re-listed a session that had already ended.
        let changed = r.apply_hook(&json!({"hook_event_name": "Stop", "session_id": "s1"}), 3);
        assert!(!changed, "a straggler must not be an observable change");
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Ended);
        assert!(!r.is_bound("s1"), "a straggler Stop must not re-list an ended session");

        let changed2 = r.apply_hook(
            &json!({"hook_event_name": "PostToolUseFailure", "session_id": "s1", "error": "boom", "is_interrupt": false}),
            4,
        );
        assert!(!changed2);
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Ended);
        assert!(!r.is_bound("s1"), "a straggler failure must not re-list an ended session either");
    }

    #[test]
    fn a_session_start_after_session_end_revives_and_rejoins_the_list() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 2);
        assert!(!r.is_bound("s1"));

        let changed = r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "resume", "session_id": "s1"}), 3);
        assert!(changed);
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Idle);
        assert!(r.is_bound("s1"), "a resume after SessionEnd rejoins the list");
    }

    #[test]
    fn enumerated_sessions_do_not_overwrite_observed_state() {
        let mut r = Registry::default();
        r.apply_hook(
            &json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}),
            1,
        );
        // A name does legitimately fill in s1's still-blank label here
        // (the 2026-09-15 label fix), so this no longer asserts the call
        // changed nothing at all; state is the thing it must not touch.
        r.register_enumerated("s1", Some("name"), None, None, None, 2);
        assert_eq!(
            r.sessions["s1"].state,
            crate::state::SessionState::Thinking,
            "an enumeration row must not grey out a session with observed state"
        );
    }

    #[test]
    fn enumeration_binds_a_session_a_hook_never_reported() {
        // Enumeration is the only channel that ever sees a session in a
        // repo with no hook wired up.
        let mut r = Registry::default();
        assert!(r.register_enumerated("other-repo", Some("undertow"), Some("C:/dev/undertow"), Some(1), None, 1));
        assert!(r.is_bound("other-repo"));
        assert_eq!(r.sessions["other-repo"].state, crate::state::SessionState::Unknown);
    }

    #[test]
    fn events_without_a_session_id_are_dropped() {
        let mut r = Registry::default();
        assert!(!r.apply_hook(&json!({"hook_event_name": "Stop"}), 1));
        assert!(r.sessions.is_empty());
    }

    #[test]
    fn select_on_a_bound_row_returns_true_and_runs_on_selected() {
        let mut r = Registry::default();
        // A fresh session's own first Stop goes straight to Complete
        // (an empty child ledger), so selecting it exercises on_selected.
        r.apply_hook(&json!({"hook_event_name": "Stop", "session_id": "s1"}), 1);
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Complete);
        assert!(r.select(0, 2));
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Idle, "on_selected clears complete to idle");
        assert!(r.sessions["s1"].unread_since_ms.is_none(), "on_selected clears the unread mark");
    }

    #[test]
    fn select_out_of_range_returns_false() {
        let mut r = Registry::default();
        assert!(!r.select(0, 1), "an empty list has no row 0 to select");
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        assert!(!r.select(5, 2), "there is no row past the end of the list");
    }

    #[test]
    fn tick_flips_a_quiet_session_to_unknown_once() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        assert!(r.tick(1 + crate::state::T_UNKNOWN_MS + 1));
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Unknown);
        assert!(!r.tick(1 + crate::state::T_UNKNOWN_MS + 2), "an immediate second tick has nothing left to flip");
    }

    #[test]
    fn snapshot_has_one_tile_per_bound_session_with_correct_indices_and_selected_flag() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s2"}), 1);
        r.select(0, 2);
        let snap = r.snapshot(3);
        assert_eq!(snap.tiles.len(), 2);
        for (i, t) in snap.tiles.iter().enumerate() {
            assert_eq!(t.index, i);
            assert_eq!(t.selected, i == 0);
            assert!(t.session.is_some());
        }
    }

    #[test]
    fn snapshot_on_an_empty_registry_has_no_tiles() {
        let r = Registry::default();
        assert!(r.snapshot(1).tiles.is_empty());
    }

    #[test]
    fn snapshot_carries_hide_unknown_and_still_lists_every_tile() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.hide_unknown = true;
        let snap = r.snapshot(2);
        assert!(snap.hide_unknown);
        assert_eq!(snap.tiles.len(), 1, "hide_unknown must not remove a tile from the snapshot itself");
    }

    #[test]
    fn register_enumerated_corrects_a_cwd_poisoned_by_a_subagent_payload() {
        // state.rs now refuses to latch a subagent's cwd onto a session,
        // but this pins the repair path for a value that got in before
        // that fix, or by any other means: enumeration's cwd is the
        // session's own and always wins.
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        r.sessions.get_mut("s1").unwrap().cwd = Some("C:/dev/wrong-subagent-dir".to_string());
        assert!(r.register_enumerated("s1", None, Some("C:/dev/undertow"), None, None, 2));
        assert_eq!(r.sessions["s1"].cwd.as_deref(), Some("C:/dev/undertow"));
    }

    #[test]
    fn register_enumerated_fills_a_blank_label_from_cwd_but_never_overwrites_one() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        assert_eq!(r.sessions["s1"].label, "");
        r.register_enumerated("s1", None, Some("C:/dev/undertow"), None, None, 2);
        assert_eq!(r.sessions["s1"].label, "undertow", "a blank label is filled in from the corrected cwd");

        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s2"}), 1);
        r.sessions.get_mut("s2").unwrap().label = "custom name".to_string();
        r.register_enumerated("s2", None, Some("C:/dev/undertow"), None, None, 2);
        assert_eq!(r.sessions["s2"].label, "custom name", "a non-derived label is never overwritten");

        // The 2026-09-15 report: a hook-first session's label came only
        // from cwd (a derived label), while a scan-first session's came
        // from the scan's own `name`, and the two never agreed even
        // though they named the same session. A label the hook itself
        // derived from cwd stays open to a later scan's real name,
        // tracked by `label_is_derived` rather than by comparing
        // strings, since a session's real name could coincidentally
        // equal its directory's.
        r.apply_hook(
            &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s3", "cwd": "C:/dev/deckhand"}),
            1,
        );
        assert_eq!(r.sessions["s3"].label, "deckhand", "the hook derives a label from cwd exactly like enumeration does");
        r.register_enumerated("s3", Some("deckhand-e4"), None, None, None, 2);
        assert_eq!(r.sessions["s3"].label, "deckhand-e4", "a scan name replaces a label that was only ever derived from cwd");

        // Once a real name has been seen, it is never overwritten again,
        // including by a later cwd-only row.
        r.register_enumerated("s3", None, Some("C:/dev/deckhand-renamed"), None, None, 3);
        assert_eq!(r.sessions["s3"].label, "deckhand-e4", "a real name outlives a later cwd-only scan row");

        // The equality case (2026-09-15 review): the scan's real name is
        // the directory name, character for character. The source still
        // has to win over the string, or a session that was hook-first
        // stays replaceable while its scan-first twin does not.
        r.apply_hook(
            &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s4", "cwd": "C:/dev/deckhand"}),
            1,
        );
        assert!(r.sessions["s4"].label_is_derived);
        assert!(
            r.register_enumerated("s4", Some("deckhand"), Some("C:/dev/deckhand"), None, None, 2),
            "accepting a real name is a change even when its text already matches"
        );
        assert!(!r.sessions["s4"].label_is_derived, "the flag clears on the name's source, not its text");
        assert!(
            !r.register_enumerated("s4", Some("deckhand"), Some("C:/dev/deckhand"), None, None, 3),
            "the same row again is a no-op"
        );
        r.register_enumerated("s4", None, Some("C:/dev/deckhand-renamed"), None, None, 4);
        assert_eq!(
            r.sessions["s4"].label,
            "deckhand",
            "a real name equal to the old directory name still outlives a cwd-only row"
        );
    }

    // ---- register_enumerated: pid replacement policy --------------------
    //
    // A non-null enumerated pid different from the stored one replaces it
    // and reports changed; a non-null pid equal to the stored one is a
    // no-op; a missing pid (the scan saw none this time) preserves
    // whatever is already recorded.

    #[test]
    fn register_enumerated_replaces_a_changed_pid_and_preserves_label_and_state() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "cwd": "C:/dev/a"}), 1);
        r.sessions.get_mut("s1").unwrap().pid = Some(111);
        assert!(r.register_enumerated("s1", None, None, Some(222), None, 2));
        assert_eq!(r.sessions["s1"].pid, Some(222), "a different enumerated pid replaces the stored one");
        assert_eq!(r.sessions["s1"].label, "a", "replacing the pid must not disturb the label");
        assert_eq!(
            r.sessions["s1"].state,
            crate::state::SessionState::Thinking,
            "replacing the pid must not disturb state"
        );
    }

    #[test]
    fn register_enumerated_is_a_no_op_when_the_enumerated_pid_matches() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        r.sessions.get_mut("s1").unwrap().pid = Some(111);
        assert!(!r.register_enumerated("s1", None, None, Some(111), None, 2), "the same pid must report no change");
        assert_eq!(r.sessions["s1"].pid, Some(111));
    }

    #[test]
    fn register_enumerated_with_no_pid_preserves_the_stored_one() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        r.sessions.get_mut("s1").unwrap().pid = Some(111);
        assert!(!r.register_enumerated("s1", None, None, None, None, 2), "a scan that saw no pid must not clear it");
        assert_eq!(r.sessions["s1"].pid, Some(111));
    }

    #[test]
    fn a_resumed_session_gets_the_new_enumerated_pid_after_end_and_resume() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.register_enumerated("s1", None, None, Some(111), None, 2);
        assert_eq!(r.sessions["s1"].pid, Some(111));

        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 3);
        assert_eq!(r.sessions["s1"].pid, None, "SessionEnd clears the pid: the OS may reuse it for anything");

        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "resume", "session_id": "s1"}), 4);
        assert!(r.is_bound("s1"), "the revived session rejoins the list");

        assert!(r.register_enumerated("s1", None, None, Some(222), None, 5));
        assert_eq!(r.sessions["s1"].pid, Some(222), "the new OS process's pid replaces the stale one");
    }

    #[test]
    fn an_ended_session_listed_by_a_later_scan_stays_unbound_with_no_pid() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.register_enumerated("s1", None, None, Some(111), None, 2);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 3);
        assert_eq!(r.sessions["s1"].pid, None);

        // The OS could have reused pid 111 for an unrelated process by
        // the time the next scan runs; enumeration must not hand a pid
        // back to a session that has already ended.
        assert!(!r.register_enumerated("s1", None, None, Some(999), None, 4));
        assert!(!r.is_bound("s1"));
        assert_eq!(r.sessions["s1"].pid, None, "an ended session's pid must not be repopulated by a later scan");
    }

    // ---- prune_missing: enumeration lag and the failed-run rule --------

    #[test]
    fn prune_drops_a_bound_session_absent_from_a_successful_enumeration() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let present = HashSet::new();
        assert!(r.prune_missing(&present, 1 + ENUM_GRACE_MS + 1));
        assert!(!r.is_bound("s1"));
        assert!(r.sessions.contains_key("s1"), "pruning drops the binding, not the session record");
    }

    #[test]
    fn prune_keeps_a_session_present_in_the_enumeration() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let present: HashSet<String> = ["s1".to_string()].into_iter().collect();
        assert!(!r.prune_missing(&present, 1 + ENUM_GRACE_MS + 1));
        assert!(r.is_bound("s1"));
    }

    #[test]
    fn prune_keeps_a_session_missing_from_enumeration_but_heard_from_recently() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 100);
        let present = HashSet::new();
        // Well within the grace window since the last hook at t=100.
        assert!(!r.prune_missing(&present, 100 + ENUM_GRACE_MS - 1));
        assert!(r.is_bound("s1"), "a session heard from moments ago must survive a lagging enumeration");
    }

    #[test]
    fn a_failed_enumeration_must_call_prune_missing_never() {
        // This is a documentation test: the caller (enumerate::register)
        // must skip prune_missing entirely on a failed run. prune_missing
        // itself has no way to know whether the run behind `present`
        // succeeded, so the contract lives in the caller; this test pins
        // that an empty `present` set, taken at face value, would prune
        // everything, which is exactly why a failed run must never reach
        // here at all.
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let empty_present = HashSet::new();
        assert!(r.prune_missing(&empty_present, 1 + ENUM_GRACE_MS + 1));
        assert!(!r.is_bound("s1"), "an empty present-set does prune, which is why callers must gate on success");
    }

    // ---- ADR-035: liveness watches ---------------------------------
    //
    // `liveness::Fake` stands in for a real OS process handle so these
    // stay deterministic and platform-independent (the two real-handle
    // tests live in liveness.rs itself, Windows-only).

    fn fake_watch(exited: bool) -> (liveness::Watch, std::sync::Arc<std::sync::atomic::AtomicBool>) {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(exited));
        let watch: liveness::Watch = Box::new(liveness::Fake(flag.clone()));
        (watch, flag)
    }

    #[test]
    fn a_session_with_a_live_watch_survives_a_scan_that_omits_it() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let (watch, _flag) = fake_watch(false);
        r.set_watch("s1", watch);
        let present = HashSet::new();
        assert!(!r.prune_missing(&present, 1 + ENUM_GRACE_MS + 1), "a live handle beats a scan that omits the session");
        assert!(r.is_bound("s1"));
    }

    #[test]
    fn a_session_with_no_watch_is_pruned_as_before() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let present = HashSet::new();
        assert!(r.prune_missing(&present, 1 + ENUM_GRACE_MS + 1));
        assert!(!r.is_bound("s1"), "a session with no held handle is pruned exactly as before ADR-035");
    }

    #[test]
    fn an_exited_watch_ends_and_unbinds_the_session_on_tick() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        let (watch, flag) = fake_watch(false);
        r.set_watch("s1", watch);
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(r.tick(2));
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Ended);
        assert!(!r.is_bound("s1"), "a process-exit tick unbinds the row exactly like SessionEnd does");
        let snap = r.snapshot(3);
        assert!(snap.tiles.is_empty(), "the snapshot must no longer list the ended session");
    }

    #[test]
    fn a_session_start_after_an_exited_watch_revives_it() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        let (watch, _flag) = fake_watch(true);
        r.set_watch("s1", watch);
        assert!(r.tick(2));
        assert!(!r.is_bound("s1"));

        let changed = r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "resume", "session_id": "s1"}), 3);
        assert!(changed);
        assert!(r.is_bound("s1"), "only a fresh SessionStart revives a row an exited watch ended");
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Idle);
    }

    #[test]
    fn alive_idle_survives_a_twenty_minute_quiet_tick() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let (watch, _flag) = fake_watch(false);
        r.set_watch("s1", watch);
        let twenty_min = 20 * 60 * 1000;
        assert!(!r.tick(1 + twenty_min));
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Idle, "idle must not grey while the process is alive");
        assert!(r.is_bound("s1"));
    }

    #[test]
    fn a_changed_pid_replaces_the_watch() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        let (old_watch, _old_flag) = fake_watch(true);
        r.set_watch("s1", old_watch);

        // A bogus pid: liveness::open on Windows returns None for one it
        // cannot open (u32::MAX is never a real process id, and the stub
        // on other platforms always returns None), so the session simply
        // ends up with no watch at all rather than a second fake one.
        // What this pins is that the *old* watch is gone: if it were
        // still attached, the tick below would see its exited flag and
        // unbind the session.
        assert!(r.register_enumerated("s1", None, None, Some(u32::MAX), None, 2));
        assert!(r.is_bound("s1"), "replacing the pid must not itself unbind the session");
        assert!(!r.tick(3), "the stale exited watch must no longer be the one attached to this session");
        assert!(r.is_bound("s1"));
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Thinking);
    }

    // ---- note_started_at -------------------------------------------------

    #[test]
    fn note_started_at_sets_the_value_only_when_some() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        r.note_started_at("s1", Some(1000));
        assert_eq!(r.sessions["s1"].started_at_ms, Some(1000));
        r.note_started_at("s1", None);
        assert_eq!(r.sessions["s1"].started_at_ms, Some(1000), "a scan reporting no startedAt this time must not clear it");
    }

    #[test]
    fn note_started_at_on_an_unknown_id_is_a_harmless_no_op() {
        let mut r = Registry::default();
        assert!(!r.note_started_at("nobody", Some(1000)));
        assert!(r.sessions.is_empty());
    }

    #[test]
    fn note_started_at_reports_a_change_only_when_the_stored_value_moves() {
        // enumerate::register folds this into its change flag, so a
        // repeated identical scan must not keep the scan loop
        // repainting.
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        assert!(r.note_started_at("s1", Some(1000)), "first sighting of a startedAt is a change");
        assert!(!r.note_started_at("s1", Some(1000)), "the same value again is not");
        assert!(!r.note_started_at("s1", None), "a scan without one changes nothing");
        assert!(r.note_started_at("s1", Some(2000)), "a moved value is a change");
        assert_eq!(r.sessions["s1"].started_at_ms, Some(2000));
    }

    #[test]
    fn note_started_at_leaves_an_ended_session_alone() {
        // register_enumerated leaves an ended session untouched entirely;
        // the scan can keep listing its process, and this must not
        // report a change for a row that will never show.
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "session_id": "s1"}), 2);
        assert_eq!(r.sessions["s1"].state, crate::state::SessionState::Ended);
        assert!(!r.note_started_at("s1", Some(1000)));
        assert_eq!(r.sessions["s1"].started_at_ms, None);
    }

    // ---- supersession: snapshot and the row count both hide it ----------
    //
    // The pure decision itself (every edge case) is pinned in
    // supersede.rs; these pin only that the registry actually wires it
    // in at the two places that matter -- the tiles the surface renders
    // (snapshot) and the row count the window is sized off
    // (main.rs::visible_rows, exercised here through superseded_ids
    // directly since visible_rows itself lives outside this crate's
    // test reach) -- and that a hidden session keeps its binding.

    /// Bind a session with a hook, then give it a pid, a VS Code host,
    /// and a parent pid directly (bypassing a real Toolhelp32 walk, the
    /// same way these tests already fake a liveness watch instead of a
    /// real process handle).
    fn bind_vscode_session(r: &mut Registry, id: &str, pid: u32, parent_pid: u32, cwd: &str, now_ms: i64) {
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id, "cwd": cwd}), now_ms);
        let s = r.sessions.get_mut(id).unwrap();
        s.pid = Some(pid);
        s.host = Some(crate::host::Host::VsCode);
        s.parent_pid = Some(parent_pid);
    }

    #[test]
    fn snapshot_hides_a_superseded_session_but_keeps_its_binding() {
        let mut r = Registry::default();
        bind_vscode_session(&mut r, "old", 1, 100, "C:/dev/deckhand", 1);
        bind_vscode_session(&mut r, "new", 2, 100, "C:/dev/deckhand", 2);
        // "old" is idle from SessionStart; "new" needs an event of its
        // own to move off idle and past "old"'s first-seen time.
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "new"}), 3);

        let snap = r.snapshot(4);
        let ids: Vec<Option<String>> = snap.tiles.iter().map(|t| t.session.as_ref().map(|s| s.id.clone())).collect();
        assert_eq!(ids, vec![Some("new".to_string())], "old is hidden from the tiles the surface renders");
        assert!(r.is_bound("old"), "old keeps its place in bindings; it is hidden, not unbound");
        assert!(
            r.superseded_ids().contains("old"),
            "main.rs::visible_rows sizes the window off this same set"
        );
    }

    #[test]
    fn a_superseded_session_reappears_once_it_stops_qualifying() {
        let mut r = Registry::default();
        bind_vscode_session(&mut r, "old", 1, 100, "C:/dev/deckhand", 1);
        bind_vscode_session(&mut r, "new", 2, 100, "C:/dev/deckhand", 2);
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "new"}), 3);
        assert_eq!(r.snapshot(4).tiles.len(), 1, "old starts hidden");

        // A hook arrives for old: it is no longer idle, so it is no
        // longer safe to hide, with nothing else needing to change.
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "old"}), 5);
        let snap = r.snapshot(6);
        assert_eq!(snap.tiles.len(), 2, "old shows again with no extra bookkeeping");
    }

    #[test]
    fn a_selected_row_that_becomes_superseded_disappears_and_returns_selected() {
        let mut r = Registry::default();
        bind_vscode_session(&mut r, "old", 1, 100, "C:/dev/deckhand", 1);
        assert!(r.select(0, 2));

        bind_vscode_session(&mut r, "new", 2, 100, "C:/dev/deckhand", 3);
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "new"}), 4);

        let snap = r.snapshot(5);
        assert!(snap.tiles.iter().all(|t| !t.selected), "the selected row is hidden along with the session");

        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "old"}), 6);
        let snap2 = r.snapshot(7);
        let old_tile = snap2.tiles.iter().find(|t| t.session.as_ref().map(|s| s.id.as_str()) == Some("old")).unwrap();
        assert!(old_tile.selected, "the old selection survives the round trip with no extra bookkeeping");
    }

    #[test]
    fn a_session_with_no_pid_is_never_superseded_through_the_registry() {
        // A sanity check that the registry actually passes `pid` through
        // to the candidate (supersede.rs pins the decision rule itself):
        // "old" here never got a pid, so it must never be hidden no
        // matter how good every other match is.
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "old", "cwd": "C:/dev/deckhand"}), 1);
        bind_vscode_session(&mut r, "new", 2, 100, "C:/dev/deckhand", 2);
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "new"}), 3);
        assert_eq!(r.snapshot(4).tiles.len(), 2, "old has no pid, so it is never a supersession candidate");
    }

    // ---- register_enumerated: host and parent pid resolution -------------

    #[test]
    fn a_pid_learned_by_enumeration_gets_a_host_classification() {
        // host::resolve runs a real (Windows) process snapshot; this
        // only pins that register_enumerated actually calls it and
        // stores *something* rather than leaving the fields at their
        // Session::new default of None. The decision rule itself never
        // depends on which concrete Host a real process resolves to.
        let mut r = Registry::default();
        assert!(r.register_enumerated("s1", None, None, Some(std::process::id()), None, 1));
        assert!(r.sessions["s1"].host.is_some(), "a learned pid must get a host classification");
    }
}
