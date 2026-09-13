// Local persistence: the daemon's contact file (read by the shim) and
// the tile bindings, both under %LOCALAPPDATA%\deckhand. Bindings are by
// session id, which survives restarts. Nothing leaves the machine.
//
// Every public function resolves data_dir() and delegates to a `_in`
// variant that takes the directory explicitly, so tests can point at a
// throwaway directory instead of the real LOCALAPPDATA.

use std::fs;
use std::path::{Path, PathBuf};

use crate::registry::Registry;

pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = PathBuf::from(base).join("deckhand");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Written at startup so the shim can find the daemon; removed on clean
/// shutdown. A stale file after a crash is harmless: the shim's connect
/// fails fast and it exits silently.
pub fn write_daemon_contact(port: u16, token: &str) {
    if let Some(dir) = data_dir() {
        write_daemon_contact_in(&dir, port, token);
    }
}

/// The shim parses this file by hand, not through a JSON library, so the
/// exact byte shape is a contract and not just an encoding choice.
pub fn write_daemon_contact_in(dir: &Path, port: u16, token: &str) {
    let body = format!("{{\"port\":{port},\"token\":\"{token}\"}}");
    let _ = fs::write(dir.join("daemon.json"), body);
}

pub fn remove_daemon_contact() {
    if let Some(dir) = data_dir() {
        remove_daemon_contact_in(&dir);
    }
}

pub fn remove_daemon_contact_in(dir: &Path) {
    let _ = fs::remove_file(dir.join("daemon.json"));
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedBinding {
    id: String,
    label: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WindowPos {
    pub x: i32,
    pub y: i32,
}

pub fn save_window_pos(x: i32, y: i32) {
    if let Some(dir) = data_dir() {
        save_window_pos_in(&dir, x, y);
    }
}

pub fn save_window_pos_in(dir: &Path, x: i32, y: i32) {
    if let Ok(body) = serde_json::to_string(&WindowPos { x, y }) {
        let _ = fs::write(dir.join("window.json"), body);
    }
}

pub fn load_window_pos() -> Option<WindowPos> {
    let dir = data_dir()?;
    load_window_pos_in(&dir)
}

pub fn load_window_pos_in(dir: &Path) -> Option<WindowPos> {
    let body = fs::read_to_string(dir.join("window.json")).ok()?;
    serde_json::from_str(&body).ok()
}

/// The list is ordered and unbounded: index is row position, not a slot
/// out of a fixed count. Every entry is a real binding; there are no
/// gaps in a file this build writes.
pub fn save_bindings(reg: &Registry) {
    let Some(dir) = data_dir() else { return };
    save_bindings_in(&dir, reg);
}

pub fn save_bindings_in(dir: &Path, reg: &Registry) {
    let list: Vec<SavedBinding> = reg
        .bindings
        .iter()
        .map(|id| SavedBinding {
            id: id.clone(),
            label: reg
                .sessions
                .get(id)
                .map(|s| s.label.clone())
                .unwrap_or_default(),
        })
        .collect();
    if let Ok(body) = serde_json::to_string(&list) {
        let _ = fs::write(dir.join("bindings.json"), body);
    }
}

/// Restore bindings and materialise a placeholder session for any bound
/// id the daemon has not seen: state unknown, saved label. That is the
/// cold-start promise (docs/ARCHITECTURE.md): the rows are the right
/// rows under the right names, and they are grey until an event or the
/// enumeration says more. A bound row must never disappear just because
/// the daemon restarted.
///
/// The file this build writes is a plain array of `{id,label}`, but a
/// pre-list build wrote a fixed six-element array with `null` for an
/// empty slot; that shape still parses here (each slot is
/// `Option<SavedBinding>`) and the nulls are simply dropped, so a
/// daemon upgraded in place keeps whatever it had bound, minus the gaps.
pub fn load_bindings(reg: &mut Registry, now_ms: i64) {
    let Some(dir) = data_dir() else { return };
    load_bindings_in(&dir, reg, now_ms);
}

pub fn load_bindings_in(dir: &Path, reg: &mut Registry, now_ms: i64) {
    let Ok(body) = fs::read_to_string(dir.join("bindings.json")) else {
        return;
    };
    let Ok(list) = serde_json::from_str::<Vec<Option<SavedBinding>>>(&body) else {
        return;
    };
    for saved in list.into_iter().flatten() {
        reg.ensure_session(&saved.id, &saved.label, now_ms);
        if !reg.bindings.contains(&saved.id) {
            reg.bindings.push(saved.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    // A fresh, unique directory per test, under the OS temp dir rather
    // than data_dir(), so tests never touch a real %LOCALAPPDATA%\deckhand
    // and never collide with each other when run in parallel.
    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("deckhand-persist-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn contact_file_bytes_are_the_exact_shim_contract() {
        let dir = temp_dir();
        write_daemon_contact_in(&dir, 4242, "abc123");
        let body = fs::read_to_string(dir.join("daemon.json")).unwrap();
        assert_eq!(body, "{\"port\":4242,\"token\":\"abc123\"}");
        cleanup(&dir);
    }

    #[test]
    fn remove_is_idempotent_when_the_file_is_absent() {
        let dir = temp_dir();
        remove_daemon_contact_in(&dir); // no file yet, must not panic
        remove_daemon_contact_in(&dir); // still no file, must not panic
        cleanup(&dir);
    }

    #[test]
    fn window_pos_round_trips() {
        let dir = temp_dir();
        save_window_pos_in(&dir, 10, -20);
        let pos = load_window_pos_in(&dir).unwrap();
        assert_eq!((pos.x, pos.y), (10, -20));
        cleanup(&dir);
    }

    #[test]
    fn load_with_no_file_gives_none() {
        let dir = temp_dir();
        assert!(load_window_pos_in(&dir).is_none());
        cleanup(&dir);
    }

    #[test]
    fn bindings_round_trip_preserves_order_id_and_label() {
        let dir = temp_dir();
        let mut reg = Registry::default();
        reg.ensure_session("s1", "My Label", 1);
        reg.bindings.push("s1".to_string());
        save_bindings_in(&dir, &reg);

        let mut reg2 = Registry::default();
        load_bindings_in(&dir, &mut reg2, 5);
        assert_eq!(reg2.bindings, vec!["s1".to_string()]);
        assert_eq!(reg2.sessions["s1"].label, "My Label");
        cleanup(&dir);
    }

    #[test]
    fn a_bound_id_the_registry_has_never_seen_materialises_as_unknown_with_the_saved_label() {
        let dir = temp_dir();
        // The legacy shape: a fixed six-element array with null gaps.
        let saved: Vec<Option<SavedBinding>> = vec![
            None,
            Some(SavedBinding { id: "ghost".into(), label: "Ghost Session".into() }),
            None,
            None,
            None,
            None,
        ];
        fs::write(dir.join("bindings.json"), serde_json::to_string(&saved).unwrap()).unwrap();

        let mut reg = Registry::default();
        load_bindings_in(&dir, &mut reg, 99);
        assert_eq!(reg.bindings, vec!["ghost".to_string()], "the null gaps must be filtered, not preserved as empty rows");
        let s = &reg.sessions["ghost"];
        assert_eq!(s.label, "Ghost Session");
        assert_eq!(s.state, crate::state::SessionState::Unknown, "the cold start promise: grey, never guessed");
        cleanup(&dir);
    }

    #[test]
    fn a_saved_list_longer_than_the_old_six_slot_limit_loads_in_full() {
        // The whole point of the list: there is no fixed count to
        // truncate against any more.
        let dir = temp_dir();
        let saved: Vec<Option<SavedBinding>> = (0..9)
            .map(|i| Some(SavedBinding { id: format!("s{i}"), label: format!("L{i}") }))
            .collect();
        fs::write(dir.join("bindings.json"), serde_json::to_string(&saved).unwrap()).unwrap();

        let mut reg = Registry::default();
        load_bindings_in(&dir, &mut reg, 1);
        let expected: Vec<String> = (0..9).map(|i| format!("s{i}")).collect();
        assert_eq!(reg.bindings, expected);
        assert_eq!(reg.sessions.len(), 9);
        cleanup(&dir);
    }

    #[test]
    fn legacy_null_slots_are_dropped_and_the_remaining_order_is_kept() {
        let dir = temp_dir();
        let saved: Vec<Option<SavedBinding>> = vec![
            Some(SavedBinding { id: "s1".into(), label: "L1".into() }),
            None,
            Some(SavedBinding { id: "s2".into(), label: "L2".into() }),
            None,
            None,
            None,
        ];
        fs::write(dir.join("bindings.json"), serde_json::to_string(&saved).unwrap()).unwrap();

        let mut reg = Registry::default();
        load_bindings_in(&dir, &mut reg, 1);
        assert_eq!(reg.bindings, vec!["s1".to_string(), "s2".to_string()]);
        cleanup(&dir);
    }

    #[test]
    fn corrupt_json_leaves_the_registry_untouched() {
        let dir = temp_dir();
        fs::write(dir.join("bindings.json"), "not json").unwrap();

        let mut reg = Registry::default();
        load_bindings_in(&dir, &mut reg, 1);
        assert!(reg.sessions.is_empty());
        assert!(reg.bindings.is_empty());
        cleanup(&dir);
    }
}
