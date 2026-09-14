// The session registry: every session the daemon knows about, the
// ordered list of sessions the surface shows, and the snapshot the
// surface renders. Bindings are by session id, which survives restarts
// (docs/ARCHITECTURE.md#persistence).
//
// The list auto-binds: any session heard from, by a hook event or by
// enumeration, appends to it the first time it is seen, in the order it
// was seen. There is no picker and no fixed slot count. A session drops
// out of the list once it reaches `ended`, and enumeration prunes a
// bound session that has gone missing from a successful run, so long as
// it has not heard from a hook recently (enumeration can lag a live
// event by a beat or two).

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use crate::state::{Session, SessionState};

/// A session that vanished from a successful enumeration less than this
/// long ago is kept rather than pruned: enumeration and the hook stream
/// are two independent channels, and a live hook is the more trustworthy
/// of the two when they briefly disagree.
pub const ENUM_GRACE_MS: i64 = 60_000;

#[derive(Debug, Default)]
pub struct Registry {
    pub sessions: HashMap<String, Session>,
    /// The ordered, unbounded list of bound session ids. Index into this
    /// is the row index the surface renders and the index select_tile
    /// and reveal_session take.
    pub bindings: Vec<String>,
    pub selected: Option<usize>,
    /// The header's grey toggle. Owned here so `snapshot` can report it
    /// and so a resize can be sized off the visible row count; persisted
    /// by persist.rs, not here.
    pub hide_unknown: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSnapshot {
    pub index: usize,
    pub selected: bool,
    pub session: Option<Session>,
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
        // outright rather than lingering as a dead row.
        let list_changed = if ended { self.unbind_id(id) } else { self.auto_bind(id) };
        changed || list_changed
    }

    /// Register a session found by enumeration (`claude agents --json`).
    /// State is unknown by rule: the enumeration carries no status on
    /// 2.1.220 and idle is never guessed (ADR-024, adapter rule 1). Also
    /// auto-binds: enumeration is the only channel that ever sees a
    /// session in another repo, one with no hook wired up at all.
    pub fn register_enumerated(
        &mut self,
        id: &str,
        name: Option<&str>,
        cwd: Option<&str>,
        pid: Option<u32>,
        now_ms: i64,
    ) -> bool {
        let mut changed = false;
        if let Some(existing) = self.sessions.get_mut(id) {
            // State is never taken from the enumeration, but a pid is:
            // hooks cannot carry one and Reveal wants it.
            if existing.pid.is_none() && pid.is_some() {
                existing.pid = pid;
                changed = true;
            }
            // Unlike pid, cwd is corrected rather than only filled in:
            // enumeration reports the session's own cwd, so it is the
            // one channel that can repair a value a subagent payload
            // latched onto the session before this fix (state.rs
            // apply_hook). Label follows the same existing-name
            // precedence as everywhere else: only filled in when blank,
            // never overwritten.
            if let Some(cwd) = cwd {
                if existing.cwd.as_deref() != Some(cwd) {
                    existing.cwd = Some(cwd.to_string());
                    changed = true;
                }
                if existing.label.is_empty() {
                    existing.label = crate::state::dir_name(cwd);
                    changed = true;
                }
            }
            // A session the hooks already saw end stays off the list
            // even if the enumeration lags behind and still reports it.
            if existing.state == SessionState::Ended {
                return changed;
            }
        } else {
            let mut s = Session::new(id.to_string(), now_ms);
            s.pid = pid;
            if let Some(cwd) = cwd {
                s.cwd = Some(cwd.to_string());
                s.label = crate::state::dir_name(cwd);
            }
            if let Some(n) = name {
                if !n.is_empty() {
                    s.label = n.to_string();
                }
            }
            self.sessions.insert(id.to_string(), s);
            changed = true;
        }
        changed |= self.auto_bind(id);
        changed
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
    /// they received a hook event within `ENUM_GRACE_MS`. Must only be
    /// called after an enumeration run that actually succeeded: a failed
    /// run (claude missing, unparseable output) carries no information
    /// about who is still alive and must prune nothing.
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
            .cloned()
            .collect();
        let mut changed = false;
        for id in stale {
            changed |= self.unbind_id(&id);
        }
        changed
    }

    pub fn select(&mut self, index: usize, now_ms: i64) -> bool {
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

    pub fn tick(&mut self, now_ms: i64) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            changed |= s.tick(now_ms);
        }
        changed
    }

    pub fn snapshot(&self, now_ms: i64) -> Snapshot {
        Snapshot {
            tiles: self
                .bindings
                .iter()
                .enumerate()
                .map(|(i, id)| TileSnapshot {
                    index: i,
                    selected: self.selected == Some(i),
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

    #[test]
    fn a_lagging_enumeration_does_not_rebind_an_ended_session() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "s1"}), 2);
        // Recording the pid is a real change; the row must still not return.
        r.register_enumerated("s1", None, None, Some(7), 3);
        assert!(!r.is_bound("s1"), "hooks saw it end; the enumeration is stale");
    }

    #[test]
    fn enumerated_sessions_do_not_overwrite_observed_ones() {
        let mut r = Registry::default();
        r.apply_hook(
            &json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}),
            1,
        );
        assert!(!r.register_enumerated("s1", Some("name"), None, None, 2));
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
        assert!(r.register_enumerated("other-repo", Some("undertow"), Some("C:/dev/undertow"), Some(1), 1));
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
        assert!(r.register_enumerated("s1", None, Some("C:/dev/undertow"), None, 2));
        assert_eq!(r.sessions["s1"].cwd.as_deref(), Some("C:/dev/undertow"));
    }

    #[test]
    fn register_enumerated_fills_a_blank_label_from_cwd_but_never_overwrites_one() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1"}), 1);
        assert_eq!(r.sessions["s1"].label, "");
        r.register_enumerated("s1", None, Some("C:/dev/undertow"), None, 2);
        assert_eq!(r.sessions["s1"].label, "undertow", "a blank label is filled in from the corrected cwd");

        r.apply_hook(&json!({"hook_event_name": "UserPromptSubmit", "session_id": "s2"}), 1);
        r.sessions.get_mut("s2").unwrap().label = "custom name".to_string();
        r.register_enumerated("s2", None, Some("C:/dev/undertow"), None, 2);
        assert_eq!(r.sessions["s2"].label, "custom name", "an existing label is never overwritten");
    }

    #[test]
    fn register_enumerated_never_overwrites_a_pid_already_present() {
        let mut r = Registry::default();
        assert!(r.register_enumerated("s1", None, None, Some(111), 1));
        assert!(!r.register_enumerated("s1", None, None, Some(222), 2), "a second pid on a known id changes nothing");
        assert_eq!(r.sessions["s1"].pid, Some(111));
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
}
