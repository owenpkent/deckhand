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
        now_ms: i64,
    ) -> bool {
        if self.sessions.contains_key(id) {
            return false;
        }
        let mut s = Session::new(id.to_string(), now_ms);
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
        assert!(!r.register_enumerated("s1", Some("name"), None, 2));
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
}
