// Deckhand: daemon and surface in one Tauri application
// (docs/ARCHITECTURE.md#processes). The Rust side is the daemon: it owns
// every session state machine and the ingest endpoint. The webview draws
// tiles and sends intents, and holds no authority and no inference.
//
// Phase 1 is observation only. Nothing in this process can approve,
// deny, send, or interrupt anything.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod enumerate;
mod http;
mod persist;
mod registry;
mod state;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager, State};

struct Shared(Arc<Mutex<registry::Registry>>);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn emit_snapshot(app: &tauri::AppHandle, reg: &registry::Registry) {
    let _ = app.emit("deckhand://snapshot", reg.snapshot(now_ms()));
}

#[cfg(windows)]
mod win_style {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const WS_EX_TOPMOST: isize = 0x0000_0008;

    /// The mechanism proven by the Phase 0 spike and recorded in ADR-025:
    /// Tauri's own options supply topmost but not no-activate, so the
    /// missing extended-style bit is set here once at setup.
    pub fn apply_noactivate(hwnd: isize) {
        unsafe {
            let before = GetWindowLongPtrW(hwnd as _, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd as _,
                GWL_EXSTYLE,
                before | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            );
            SetWindowPos(
                hwnd as _,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

#[tauri::command]
fn snapshot(shared: State<Shared>) -> registry::Snapshot {
    shared.0.lock().unwrap().snapshot(now_ms())
}

#[tauri::command]
fn select_tile(index: usize, shared: State<Shared>, app: tauri::AppHandle) {
    let mut reg = shared.0.lock().unwrap();
    if reg.select(index, now_ms()) {
        emit_snapshot(&app, &reg);
    }
}

#[tauri::command]
fn bind_tile(index: usize, session_id: String, shared: State<Shared>, app: tauri::AppHandle) {
    let mut reg = shared.0.lock().unwrap();
    if reg.bind(index, &session_id, now_ms()) {
        persist::save_bindings(&reg);
        emit_snapshot(&app, &reg);
    }
}

#[tauri::command]
fn unbind_tile(index: usize, shared: State<Shared>, app: tauri::AppHandle) {
    let mut reg = shared.0.lock().unwrap();
    if reg.unbind(index) {
        persist::save_bindings(&reg);
        emit_snapshot(&app, &reg);
    }
}

#[tauri::command]
fn bindable_sessions(shared: State<Shared>) -> Vec<registry::BindableSession> {
    shared.0.lock().unwrap().bindable()
}

#[tauri::command]
fn refresh_sessions(shared: State<Shared>, app: tauri::AppHandle) {
    // Fetch without the lock: the enumeration shells out.
    let rows = enumerate::fetch().unwrap_or_default();
    let mut reg = shared.0.lock().unwrap();
    if enumerate::register(&mut reg, &rows, now_ms()) {
        emit_snapshot(&app, &reg);
    }
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            snapshot,
            select_tile,
            bind_tile,
            unbind_tile,
            bindable_sessions,
            refresh_sessions,
            quit
        ])
        .setup(|app| {
            #[cfg(windows)]
            {
                let window = app.get_webview_window("main").expect("main window");
                let hwnd = window.hwnd()?.0 as isize;
                win_style::apply_noactivate(hwnd);
            }

            let shared = Arc::new(Mutex::new(registry::Registry::default()));
            persist::load_bindings(&mut shared.lock().unwrap(), now_ms());
            app.manage(Shared(shared.clone()));

            // Ingest: shim POSTs land on this channel; one thread owns
            // the application of events so ordering is deterministic.
            let (tx, rx) = std::sync::mpsc::channel::<serde_json::Value>();
            let server = http::start(tx).expect("bind the loopback ingest endpoint");
            persist::write_daemon_contact(server.port, &server.token);

            let apply_handle = app.handle().clone();
            let apply_reg = shared.clone();
            std::thread::Builder::new()
                .name("deckhand-apply".into())
                .spawn(move || {
                    for payload in rx {
                        let mut reg = apply_reg.lock().unwrap();
                        if reg.apply_hook(&payload, now_ms()) {
                            // Auto-fill may have taken a free tile.
                            persist::save_bindings(&reg);
                            emit_snapshot(&apply_handle, &reg);
                        }
                    }
                })
                .expect("spawn apply thread");

            // T_unknown watchdog.
            let tick_handle = app.handle().clone();
            let tick_reg = shared.clone();
            std::thread::Builder::new()
                .name("deckhand-tick".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(2));
                    let mut reg = tick_reg.lock().unwrap();
                    if reg.tick(now_ms()) {
                        emit_snapshot(&tick_handle, &reg);
                    }
                })
                .expect("spawn tick thread");

            // Cold start: rebind and label from the enumeration; states
            // stay unknown until events arrive (ADR-024).
            let cold_handle = app.handle().clone();
            let cold_reg = shared.clone();
            std::thread::Builder::new()
                .name("deckhand-coldstart".into())
                .spawn(move || {
                    if let Some(rows) = enumerate::fetch() {
                        let mut reg = cold_reg.lock().unwrap();
                        if enumerate::register(&mut reg, &rows, now_ms()) {
                            emit_snapshot(&cold_handle, &reg);
                        }
                    }
                })
                .expect("spawn cold start thread");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                persist::remove_daemon_contact();
            }
        });
}
