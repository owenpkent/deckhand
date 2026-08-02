// Local persistence: the daemon's contact file (read by the shim) and
// the tile bindings, both under %LOCALAPPDATA%\deckhand. Bindings are by
// session id, which survives restarts. Nothing leaves the machine.

use std::fs;
use std::path::PathBuf;

use crate::registry::{Registry, TILE_COUNT};

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
        let body = format!("{{\"port\":{port},\"token\":\"{token}\"}}");
        let _ = fs::write(dir.join("daemon.json"), body);
    }
}

pub fn remove_daemon_contact() {
    if let Some(dir) = data_dir() {
        let _ = fs::remove_file(dir.join("daemon.json"));
    }
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
        if let Ok(body) = serde_json::to_string(&WindowPos { x, y }) {
            let _ = fs::write(dir.join("window.json"), body);
        }
    }
}

pub fn load_window_pos() -> Option<WindowPos> {
    let dir = data_dir()?;
    let body = fs::read_to_string(dir.join("window.json")).ok()?;
    serde_json::from_str(&body).ok()
}

pub fn save_bindings(reg: &Registry) {
    let Some(dir) = data_dir() else { return };
    let list: Vec<Option<SavedBinding>> = reg
        .bindings
        .iter()
        .map(|b| {
            b.as_ref().map(|id| SavedBinding {
                id: id.clone(),
                label: reg
                    .sessions
                    .get(id)
                    .map(|s| s.label.clone())
                    .unwrap_or_default(),
            })
        })
        .collect();
    if let Ok(body) = serde_json::to_string(&list) {
        let _ = fs::write(dir.join("bindings.json"), body);
    }
}

/// Restore bindings and materialise a placeholder session for any bound
/// id the daemon has not seen: state unknown, saved label. That is the
/// cold-start promise (docs/ARCHITECTURE.md): the tiles are the right
/// tiles under the right names, and they are grey until an event or the
/// enumeration says more. A bound tile must never render as unbound just
/// because the daemon restarted.
pub fn load_bindings(reg: &mut Registry, now_ms: i64) {
    let Some(dir) = data_dir() else { return };
    let Ok(body) = fs::read_to_string(dir.join("bindings.json")) else {
        return;
    };
    let Ok(list) = serde_json::from_str::<Vec<Option<SavedBinding>>>(&body) else {
        return;
    };
    for (i, saved) in list.into_iter().take(TILE_COUNT).enumerate() {
        if let Some(saved) = saved {
            reg.ensure_session(&saved.id, &saved.label, now_ms);
            reg.bindings[i] = Some(saved.id);
        }
    }
}
