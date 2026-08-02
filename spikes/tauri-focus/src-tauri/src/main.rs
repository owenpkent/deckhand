// Pre-Phase-1 spike: prove a Tauri v2 window on Windows can be
// always-on-top and clickable without ever taking foreground focus.
// See ../README.md for the pass condition.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::Manager;

static EVENTS: AtomicU64 = AtomicU64::new(0);
static SELF_HWND: AtomicI64 = AtomicI64::new(0);

const LOG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../spike-log.jsonl");

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowLongPtrW, GetWindowTextW, SetWindowLongPtrW, SetWindowPos,
        GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    // Not exposed as typed constants for GetWindowLongPtrW's isize result,
    // so spelled out here. Values are from winuser.h.
    pub const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    pub const WS_EX_TOPMOST: isize = 0x0000_0008;

    pub fn apply_noactivate(hwnd: isize) -> (isize, isize) {
        unsafe {
            let before = GetWindowLongPtrW(hwnd as _, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd as _, GWL_EXSTYLE, before | WS_EX_NOACTIVATE | WS_EX_TOPMOST);
            let after = GetWindowLongPtrW(hwnd as _, GWL_EXSTYLE);
            SetWindowPos(
                hwnd as _,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            (before, after)
        }
    }

    pub fn foreground() -> (isize, String) {
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            (hwnd as isize, String::from_utf16_lossy(&buf[..len as usize]))
        }
    }
}

fn log_line(value: &serde_json::Value) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "{value}");
    }
}

#[tauri::command]
fn record_event(kind: String) -> serde_json::Value {
    let n = EVENTS.fetch_add(1, Ordering::SeqCst) + 1;
    let (fg_hwnd, fg_title) = win::foreground();
    let self_hwnd = SELF_HWND.load(Ordering::SeqCst) as isize;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = serde_json::json!({
        "seq": n,
        "ts_ms": ts,
        "kind": kind,
        "foreground_hwnd": fg_hwnd,
        "foreground_title": fg_title,
        "self_hwnd": self_hwnd,
        "foreground_is_self": fg_hwnd == self_hwnd,
    });
    log_line(&entry);
    entry
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![record_event])
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window");
            let hwnd = window.hwnd()?.0 as isize;
            SELF_HWND.store(hwnd as i64, Ordering::SeqCst);
            let (before, after) = win::apply_noactivate(hwnd);
            log_line(&serde_json::json!({
                "kind": "setup",
                "self_hwnd": hwnd,
                "exstyle_before": format!("{before:#x}"),
                "exstyle_after": format!("{after:#x}"),
                "noactivate_already_set": before & win::WS_EX_NOACTIVATE != 0,
            }));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
