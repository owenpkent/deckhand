// The session registry: every session the daemon knows about, six tile
// bindings, and the snapshot the surface renders. Bindings are by session
// id, which survives restarts (docs/ARCHITECTURE.md#persistence).

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::state::{Session, SessionState};

pub const TILE_COUNT: usize = 6;

#[derive(Debug, Default)]
pub struct Registry {
    pub sessions: HashMap<String, Session>,
    pub bindings: [Option<String>; TILE_COUNT],
    pub selected: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSnapshot {
    pub index: usize,
    pub selected: bool,
    pub session: Option<Session>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindableSession {
    pub id: String,
    pub label: String,
    pub cwd: Option<String>,
    pub state: SessionState,
    pub bound_to: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tiles: Vec<TileSnapshot>,
    pub now_ms: i64,
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
        // A session heard from for the first time takes the first free
        // tile, so a fresh board populates itself without a picker trip.
        // Explicit bindings always win; this only fills gaps.
        if changed && !self.is_bound(id) {
            if let Some(free) = self.bindings.iter().position(Option::is_none) {
                self.bindings[free] = Some(id.to_string());
            }
        }
        changed
    }

    /// Register a session found by enumeration (`claude agents --json`).
    /// State is unknown by rule: the enumeration carries no status on
    /// 2.1.220 and idle is never guessed (ADR-024, adapter rule 1).
    pub fn register_enumerated(
        &mut self,
        id: &str,
        name: Option<&str>,
        cwd: Option<&str>,
        pid: Option<u32>,
        now_ms: i64,
    ) -> bool {
        if let Some(existing) = self.sessions.get_mut(id) {
            // State is never taken from the enumeration, but a pid is:
            // hooks cannot carry one and Reveal wants it.
            if existing.pid.is_none() && pid.is_some() {
                existing.pid = pid;
                return true;
            }
            return false;
        }
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
        true
    }

    /// Insert a placeholder for a session known only by id and label,
    /// in unknown state, without touching one that already exists.
    pub fn ensure_session(&mut self, id: &str, label: &str, now_ms: i64) {
        self.sessions.entry(id.to_string()).or_insert_with(|| {
            let mut s = Session::new(id.to_string(), now_ms);
            s.label = label.to_string();
            s
        });
    }

    pub fn is_bound(&self, id: &str) -> bool {
        self.bindings.iter().any(|b| b.as_deref() == Some(id))
    }

    pub fn select(&mut self, index: usize, now_ms: i64) -> bool {
        if index >= TILE_COUNT {
            return false;
        }
        let mut changed = self.selected != Some(index);
        self.selected = Some(index);
        if let Some(id) = self.bindings[index].clone() {
            if let Some(s) = self.sessions.get_mut(&id) {
                changed |= s.on_selected(now_ms);
            }
        }
        changed
    }

    pub fn bind(&mut self, index: usize, id: &str, now_ms: i64) -> bool {
        if index >= TILE_COUNT || !self.sessions.contains_key(id) {
            return false;
        }
        // A session lives on one tile at a time; binding moves it.
        for b in self.bindings.iter_mut() {
            if b.as_deref() == Some(id) {
                *b = None;
            }
        }
        self.bindings[index] = Some(id.to_string());
        let _ = now_ms;
        true
    }

    pub fn unbind(&mut self, index: usize) -> bool {
        if index >= TILE_COUNT || self.bindings[index].is_none() {
            return false;
        }
        self.bindings[index] = None;
        true
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
            tiles: (0..TILE_COUNT)
                .map(|i| TileSnapshot {
                    index: i,
                    selected: self.selected == Some(i),
                    session: self.bindings[i]
                        .as_ref()
                        .and_then(|id| self.sessions.get(id))
                        .cloned(),
                })
                .collect(),
            now_ms,
        }
    }

    pub fn bindable(&self) -> Vec<BindableSession> {
        let mut list: Vec<BindableSession> = self
            .sessions
            .values()
            .filter(|s| s.state != SessionState::Ended)
            .map(|s| BindableSession {
                id: s.id.clone(),
                label: s.label.clone(),
                cwd: s.cwd.clone(),
                state: s.state,
                bound_to: self
                    .bindings
                    .iter()
                    .position(|b| b.as_deref() == Some(s.id.as_str())),
            })
            .collect();
        list.sort_by(|a, b| a.label.cmp(&b.label));
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_event_auto_fills_a_free_tile() {
        let mut r = Registry::default();
        r.apply_hook(
            &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1", "cwd": "C:/dev/a"}),
            1,
        );
        assert_eq!(r.bindings[0].as_deref(), Some("s1"));
    }

    #[test]
    fn binding_moves_a_session_rather_than_duplicating_it() {
        let mut r = Registry::default();
        r.apply_hook(
            &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}),
            1,
        );
        assert!(r.bind(3, "s1", 2));
        assert_eq!(r.bindings[0], None);
        assert_eq!(r.bindings[3].as_deref(), Some("s1"));
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
    fn events_without_a_session_id_are_dropped() {
        let mut r = Registry::default();
        assert!(!r.apply_hook(&json!({"hook_event_name": "Stop"}), 1));
        assert!(r.sessions.is_empty());
    }

    #[test]
    fn select_on_a_bound_tile_returns_true_and_runs_on_selected() {
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
    fn reselecting_an_already_selected_empty_tile_returns_false() {
        // Selecting an empty tile for the first time still reports a
        // change: the highlighted tile itself moved from none to one.
        // Only a repeat select of the same already-selected empty tile
        // has nothing left to change.
        let mut r = Registry::default();
        assert!(r.select(1, 1));
        assert!(!r.select(1, 2));
    }

    #[test]
    fn bind_onto_an_occupied_tile_replaces_the_binding() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s2"}), 1);
        assert_eq!(r.bindings[0].as_deref(), Some("s1"));
        assert_eq!(r.bindings[1].as_deref(), Some("s2"));
        assert!(r.bind(0, "s2", 2));
        assert_eq!(r.bindings[0].as_deref(), Some("s2"), "tile 0 now holds s2");
        assert_eq!(r.bindings[1], None, "s2 must vacate its old tile when it moves");
    }

    #[test]
    fn unbind_returns_false_on_empty_true_on_bound_and_keeps_the_session() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        assert!(!r.unbind(3), "tile 3 was never bound");
        assert!(r.unbind(0), "s1 auto-filled tile 0");
        assert_eq!(r.bindings[0], None);
        assert!(r.sessions.contains_key("s1"), "unbinding a tile must not forget the session");
    }

    #[test]
    fn the_seventh_first_heard_session_gets_no_tile_but_is_bindable() {
        let mut r = Registry::default();
        for i in 0..7 {
            r.apply_hook(
                &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": format!("s{i}")}),
                1,
            );
        }
        assert!(r.bindings.iter().all(Option::is_some), "the first six fill every tile");
        assert!(!r.is_bound("s6"), "the seventh session has no tile");
        assert!(r.bindable().iter().any(|b| b.id == "s6"), "but it is still bindable");
    }

    #[test]
    fn bindable_includes_already_bound_sessions_with_their_tile() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        let entry = r
            .bindable()
            .into_iter()
            .find(|b| b.id == "s1")
            .expect("a bound session still appears in bindable()");
        assert_eq!(entry.bound_to, Some(0));
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
    fn snapshot_has_tile_count_tiles_with_correct_indices_and_selected_flag() {
        let mut r = Registry::default();
        r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "s1"}), 1);
        r.select(0, 2);
        let snap = r.snapshot(3);
        assert_eq!(snap.tiles.len(), TILE_COUNT);
        for (i, t) in snap.tiles.iter().enumerate() {
            assert_eq!(t.index, i);
            assert_eq!(t.selected, i == 0);
        }
    }

    #[test]
    fn register_enumerated_never_overwrites_a_pid_already_present() {
        let mut r = Registry::default();
        assert!(r.register_enumerated("s1", None, None, Some(111), 1));
        assert!(!r.register_enumerated("s1", None, None, Some(222), 2), "a second pid on a known id changes nothing");
        assert_eq!(r.sessions["s1"].pid, Some(111));
    }
}
