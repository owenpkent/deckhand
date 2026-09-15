// Local persistence: the daemon's contact file (read by the shim) and
// the tile bindings, both under %LOCALAPPDATA%\deckhand. Bindings are by
// session id, which survives restarts. Nothing leaves the machine.
//
// Every public function resolves data_dir() and delegates to a `_in`
// variant that takes the directory explicitly, so tests can point at a
// throwaway directory instead of the real LOCALAPPDATA.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::registry::Registry;

pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = PathBuf::from(base).join("deckhand");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Write `body` to `path` atomically: write to a sibling temp file, then
/// rename it over `path`. `std::fs::rename` replaces an existing
/// destination on Windows (as well as POSIX), so a reader -- the shim
/// parsing `daemon.json`, or this same process on its next cold start --
/// never observes a half-written file, whichever of the two names it
/// happens to open partway through.
///
/// The temp name is unique per call (the OS process id plus a monotonic
/// counter), not just per target file: these writes are no longer
/// serialised by the registry lock (main.rs's `after_change` builds the
/// body under that lock and writes only after it drops), so two callers
/// can legitimately target the same path around the same moment, and
/// must not share, and therefore race, the same temp file.
fn write_atomic(path: &Path, body: &str) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("deckhand");
    let tmp = path.with_file_name(format!("{file_name}.{}.{seq}.tmp", std::process::id()));
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)
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
    /// `Session::label_is_derived`, so a restored cwd-derived label stays
    /// open to the scan's real `name` after a restart. A file written
    /// before this field existed loads as derived: every label such a
    /// build saved came from either cwd or the scan's own name, and
    /// letting the scan's name replace it once is exactly the consistent
    /// outcome (ADR-038).
    #[serde(default = "default_true")]
    derived: bool,
}

/// The header's grey-hiding toggle, and the settings panel's
/// always-on-top switch ([ADR-033](../../../docs/DECISIONS.md#adr-033)).
/// `#[serde(default)]` and `#[serde(default = "default_true")]` both
/// matter here: a file written before either field existed still parses,
/// `hide_unknown` loading as false and `always_on_top` loading as true,
/// exactly the behaviour a file with no `settings.json` at all gets too.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub hide_unknown: bool,
    #[serde(default = "default_true")]
    pub always_on_top: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings { hide_unknown: false, always_on_top: true }
    }
}

pub fn save_hide_unknown(hide_unknown: bool) {
    if let Some(dir) = data_dir() {
        save_hide_unknown_in(&dir, hide_unknown);
    }
}

/// Reads the rest of the file (`always_on_top`) before overwriting it,
/// so toggling one setting never resets the other one to its default.
pub fn save_hide_unknown_in(dir: &Path, hide_unknown: bool) {
    let always_on_top = load_always_on_top_in(dir);
    if let Ok(body) = serde_json::to_string(&Settings { hide_unknown, always_on_top }) {
        let _ = write_atomic(&dir.join("settings.json"), &body);
    }
}

/// Defaults to false with no file, a corrupt file, or a file predating
/// this field: none of those are a guess, they are all the same "never
/// asked to hide anything" starting point.
pub fn load_hide_unknown() -> bool {
    let Some(dir) = data_dir() else { return false };
    load_hide_unknown_in(&dir)
}

pub fn load_hide_unknown_in(dir: &Path) -> bool {
    load_settings_in(dir).hide_unknown
}

pub fn save_always_on_top(always_on_top: bool) {
    if let Some(dir) = data_dir() {
        save_always_on_top_in(&dir, always_on_top);
    }
}

/// Reads the rest of the file (`hide_unknown`) before overwriting it,
/// for the same reason `save_hide_unknown_in` does.
pub fn save_always_on_top_in(dir: &Path, always_on_top: bool) {
    let hide_unknown = load_hide_unknown_in(dir);
    if let Ok(body) = serde_json::to_string(&Settings { hide_unknown, always_on_top }) {
        let _ = write_atomic(&dir.join("settings.json"), &body);
    }
}

/// Defaults to true (the window starts always-on-top, matching
/// `tauri.conf.json`) with no file, a corrupt file, or a file predating
/// this field.
pub fn load_always_on_top() -> bool {
    let Some(dir) = data_dir() else { return true };
    load_always_on_top_in(dir.as_path())
}

pub fn load_always_on_top_in(dir: &Path) -> bool {
    load_settings_in(dir).always_on_top
}

fn load_settings_in(dir: &Path) -> Settings {
    fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|body| serde_json::from_str::<Settings>(&body).ok())
        .unwrap_or_default()
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
    save_bindings_body_in(dir, bindings_body(reg));
}

/// The bindings list serialised ready to save, built from a live
/// Registry. Building this touches only the in-memory registry (a clone
/// of a few strings and a JSON format), no I/O, so it is safe, and
/// meant, to call while the registry lock is held. The actual disk write
/// (`save_bindings_body`/`_in`) is split out so a caller holding that
/// lock -- main.rs's `after_change`, across every thread that mutates
/// the registry -- can build the body under the lock and write it only
/// after the lock has dropped, keeping a (potentially slow) disk write
/// off it.
pub fn bindings_body(reg: &Registry) -> Option<String> {
    let list: Vec<SavedBinding> = reg
        .bindings
        .iter()
        .map(|id| {
            let session = reg.sessions.get(id);
            SavedBinding {
                id: id.clone(),
                label: session.map(|s| s.label.clone()).unwrap_or_default(),
                derived: session.map_or(true, |s| s.label_is_derived),
            }
        })
        .collect();
    serde_json::to_string(&list).ok()
}

/// Write an already-built bindings body (see `bindings_body`). `None`
/// (the registry failed to serialise, which realistically never happens
/// for these plain string fields) is a no-op rather than a panic or a
/// write of "null".
pub fn save_bindings_body(body: Option<String>) {
    let Some(dir) = data_dir() else { return };
    save_bindings_body_in(&dir, body);
}

pub fn save_bindings_body_in(dir: &Path, body: Option<String>) {
    let Some(body) = body else { return };
    let _ = write_atomic(&dir.join("bindings.json"), &body);
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
        if !reg.sessions.contains_key(&saved.id) {
            reg.ensure_session(&saved.id, &saved.label, now_ms);
            if let Some(s) = reg.sessions.get_mut(&saved.id) {
                s.label_is_derived = saved.derived;
            }
        }
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

    // ---- write_atomic ------------------------------------------------

    #[test]
    fn write_atomic_writes_a_fresh_file() {
        let dir = temp_dir();
        let path = dir.join("thing.json");
        write_atomic(&path, "first").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_replaces_rather_than_appends() {
        let dir = temp_dir();
        let path = dir.join("thing.json");
        write_atomic(&path, "first").unwrap();
        write_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second", "a second write must replace the first outright");
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_behind() {
        let dir = temp_dir();
        let path = dir.join("thing.json");
        write_atomic(&path, "content").unwrap();
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("thing.json")], "only the final file should remain");
        cleanup(&dir);
    }

    #[test]
    fn write_atomic_repeated_calls_do_not_collide_on_the_same_temp_name() {
        // Regression for the reason the temp name is unique per call, not
        // just per target path: these writes are no longer serialised by
        // the registry lock, so two callers can legitimately target the
        // same file around the same moment.
        let dir = temp_dir();
        let path = dir.join("thing.json");
        for i in 0..5 {
            write_atomic(&path, &i.to_string()).unwrap();
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), "4");
        let leftover_tmp = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!leftover_tmp, "no .tmp file should survive a successful write");
        cleanup(&dir);
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
        assert!(!reg2.sessions["s1"].label_is_derived, "a real name stays real across a restart");
        cleanup(&dir);
    }

    #[test]
    fn a_derived_label_stays_replaceable_across_a_restart_and_a_legacy_entry_loads_as_derived() {
        let dir = temp_dir();
        let mut reg = Registry::default();
        reg.ensure_session("s1", "deckhand", 1);
        reg.sessions.get_mut("s1").unwrap().label_is_derived = true;
        reg.bindings.push("s1".to_string());
        save_bindings_in(&dir, &reg);
        let mut reg2 = Registry::default();
        load_bindings_in(&dir, &mut reg2, 5);
        assert!(reg2.sessions["s1"].label_is_derived);
        cleanup(&dir);

        let dir = temp_dir();
        fs::write(dir.join("bindings.json"), r#"[{"id":"old","label":"ATDev-Marketing"}]"#).unwrap();
        let mut reg3 = Registry::default();
        load_bindings_in(&dir, &mut reg3, 5);
        assert!(reg3.sessions["old"].label_is_derived, "a file from before the flag lets the scan's name replace its label once");
        reg3.register_enumerated("old", Some("atdev-marketing-73"), None, None, None, 6);
        assert_eq!(reg3.sessions["old"].label, "atdev-marketing-73");
        cleanup(&dir);
    }

    #[test]
    fn a_bound_id_the_registry_has_never_seen_materialises_as_unknown_with_the_saved_label() {
        let dir = temp_dir();
        // The legacy shape: a fixed six-element array with null gaps.
        let saved: Vec<Option<SavedBinding>> = vec![
            None,
            Some(SavedBinding { id: "ghost".into(), label: "Ghost Session".into(), derived: false }),
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
            .map(|i| Some(SavedBinding { id: format!("s{i}"), label: format!("L{i}"), derived: false }))
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
            Some(SavedBinding { id: "s1".into(), label: "L1".into(), derived: false }),
            None,
            Some(SavedBinding { id: "s2".into(), label: "L2".into(), derived: false }),
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

    #[test]
    fn hide_unknown_round_trips() {
        let dir = temp_dir();
        save_hide_unknown_in(&dir, true);
        assert!(load_hide_unknown_in(&dir));
        cleanup(&dir);
    }

    #[test]
    fn hide_unknown_defaults_false_with_no_file() {
        let dir = temp_dir();
        assert!(!load_hide_unknown_in(&dir));
        cleanup(&dir);
    }

    #[test]
    fn an_older_settings_file_missing_the_field_loads_as_false() {
        let dir = temp_dir();
        fs::write(dir.join("settings.json"), "{}").unwrap();
        assert!(
            !load_hide_unknown_in(&dir),
            "serde default must cover a settings.json saved before this field existed"
        );
        cleanup(&dir);
    }

    #[test]
    fn corrupt_settings_json_loads_as_false_rather_than_panicking() {
        let dir = temp_dir();
        fs::write(dir.join("settings.json"), "not json").unwrap();
        assert!(!load_hide_unknown_in(&dir));
        cleanup(&dir);
    }

    // ---- always_on_top ------------------------------------------------

    #[test]
    fn always_on_top_defaults_true_with_no_file() {
        let dir = temp_dir();
        assert!(load_always_on_top_in(&dir), "the window starts always-on-top per tauri.conf.json");
        cleanup(&dir);
    }

    #[test]
    fn always_on_top_round_trips() {
        let dir = temp_dir();
        save_always_on_top_in(&dir, false);
        assert!(!load_always_on_top_in(&dir));
        save_always_on_top_in(&dir, true);
        assert!(load_always_on_top_in(&dir));
        cleanup(&dir);
    }

    #[test]
    fn an_older_settings_file_missing_always_on_top_loads_as_true() {
        let dir = temp_dir();
        // The exact shape save_hide_unknown_in wrote before this field
        // existed: only hide_unknown.
        fs::write(dir.join("settings.json"), r#"{"hide_unknown":true}"#).unwrap();
        assert!(load_always_on_top_in(&dir), "serde default must cover a settings.json saved before this field existed");
        assert!(load_hide_unknown_in(&dir), "the sibling field already in the file must still load");
        cleanup(&dir);
    }

    #[test]
    fn corrupt_settings_json_loads_always_on_top_as_true_rather_than_panicking() {
        let dir = temp_dir();
        fs::write(dir.join("settings.json"), "not json").unwrap();
        assert!(load_always_on_top_in(&dir));
        cleanup(&dir);
    }

    #[test]
    fn toggling_hide_unknown_does_not_reset_always_on_top() {
        let dir = temp_dir();
        save_always_on_top_in(&dir, false);
        save_hide_unknown_in(&dir, true);
        assert!(!load_always_on_top_in(&dir), "saving hide_unknown must preserve the sibling setting already on disk");
        assert!(load_hide_unknown_in(&dir));
        cleanup(&dir);
    }

    #[test]
    fn toggling_always_on_top_does_not_reset_hide_unknown() {
        let dir = temp_dir();
        save_hide_unknown_in(&dir, true);
        save_always_on_top_in(&dir, false);
        assert!(load_hide_unknown_in(&dir), "saving always_on_top must preserve the sibling setting already on disk");
        assert!(!load_always_on_top_in(&dir));
        cleanup(&dir);
    }
}
