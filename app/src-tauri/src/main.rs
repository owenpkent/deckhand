// Deckhand: daemon and surface in one Tauri application
// (docs/ARCHITECTURE.md#processes). The Rust side is the daemon: it owns
// every session state machine and the ingest endpoint. The webview draws
// a list of sessions and sends intents, and holds no authority and no
// inference.
//
// Phase 1 is observation only. Nothing in this process can approve,
// deny, send, or interrupt anything.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckhand::{enumerate, http, persist, registry, reveal, state, window};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager, State};

/// How often the background rescan re-runs `claude agents --json`. The
/// first run happens immediately at startup; this is the repeat
/// interval after that (cold start plus periodic rescan share one loop).
const RESCAN_INTERVAL: Duration = Duration::from_secs(15);

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

/// The row count actually on screen right now: every binding, minus the
/// ones hidden because they are unknown and the grey toggle is on. Every
/// resize must size off this, not `reg.bindings.len()`, so a toggle or a
/// session crossing into or out of unknown resizes the window exactly
/// like a binding appearing or disappearing always has.
fn visible_rows(reg: &registry::Registry) -> usize {
    window::visible_row_count(
        reg.bindings.iter().filter_map(|id| reg.sessions.get(id)).map(|s| s.state),
        reg.hide_unknown,
    )
}

/// Everything `after_change` needs from the registry, captured by
/// `prepare_change` while the registry lock is held. Building it touches
/// only the in-memory registry (string formatting and a couple of small
/// clones), never disk or the window, so it is fast and safe under the
/// lock; the caller drops the lock before passing this to `after_change`,
/// which is where the (potentially slow) disk write actually happens.
struct PendingChange {
    bindings_body: Option<String>,
    row_count: usize,
    snapshot: registry::Snapshot,
}

fn prepare_change(reg: &registry::Registry) -> PendingChange {
    PendingChange {
        bindings_body: persist::bindings_body(reg),
        row_count: visible_rows(reg),
        snapshot: reg.snapshot(now_ms()),
    }
}

/// Everything that follows a registry mutation which may have changed
/// the visible row count: persist the (now ordered) list, resize the
/// window to match, and repaint. Takes a `PendingChange` built by
/// `prepare_change` rather than the registry itself: every call site
/// builds that snapshot while it still holds the registry lock, then
/// explicitly drops the lock before calling this, so the disk write
/// below never runs while the lock is held and blocks every other
/// thread's access to the registry on it. Selection alone does not need
/// this (the visible row count is unchanged), so it calls
/// `emit_snapshot` directly instead.
///
/// The window calls must also not block this thread on the main one: the
/// resize is queued to run there later, with only the row count
/// captured.
fn after_change(app: &tauri::AppHandle, change: PendingChange) {
    persist::save_bindings_body(change.bindings_body);
    // Hook traffic calls this constantly; the window only needs touching
    // when the visible row count actually moved.
    if LAST_ROW_COUNT.swap(change.row_count, Ordering::SeqCst) != change.row_count {
        let handle = app.clone();
        let row_count = change.row_count;
        let _ = app.run_on_main_thread(move || {
            if let Some(win) = handle.get_webview_window("main") {
                resize_for_rows(&win, row_count);
            }
        });
    }
    let _ = app.emit("deckhand://snapshot", change.snapshot);
}

/// Row count the window was last sized for; `usize::MAX` until the
/// startup sizing in `setup` has run.
static LAST_ROW_COUNT: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The monitor's work area under the window right now, in physical
/// pixels. `None` when the window has no monitor at all (a headless
/// environment), which callers treat as "nothing to clamp against".
fn current_work_area(win: &tauri::WebviewWindow) -> Option<window::Rect> {
    let monitor = win.current_monitor().ok().flatten()?;
    let a = monitor.work_area();
    Some(window::Rect::new(a.position.x, a.position.y, a.size.width as i32, a.size.height as i32))
}

/// Every connected monitor's work area, in physical pixels. Used only at
/// startup to validate a saved position against the monitors actually
/// present right now (PR review P1); empty on a headless environment.
fn all_work_areas(win: &tauri::WebviewWindow) -> Vec<window::Rect> {
    win.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let a = m.work_area();
            window::Rect::new(a.position.x, a.position.y, a.size.width as i32, a.size.height as i32)
        })
        .collect()
}

/// Resize the window for the given row count and reclamp it into the
/// current monitor's work area, growing upward instead of off-screen
/// when the window sits near the bottom of the monitor. Called after
/// every registry change that can move the row count and once at
/// startup once the restored bindings are known.
fn resize_for_rows(win: &tauri::WebviewWindow, row_count: usize) {
    let Ok(scale) = win.scale_factor() else { return };
    let Some(area) = current_work_area(win) else { return };
    let (Ok(pos), Ok(outer), Ok(inner)) = (win.outer_position(), win.outer_size(), win.inner_size()) else {
        return;
    };
    // `set_size` sets the inner (client) size while `outer_size` includes
    // the invisible frame Windows keeps around even an undecorated window,
    // so the two must never be mixed: feeding the outer width back into
    // set_size grew the window by one frame on every call. The width is
    // always the constant; only the height follows the row count.
    let frame_w = outer.width as i32 - inner.width as i32;
    let frame_h = outer.height as i32 - inner.height as i32;
    let cur = window::Rect::new(pos.x, pos.y, outer.width as i32, outer.height as i32);

    let max_h_logical = ((area.h - frame_h) as f64 / scale).round() as i32;
    let new_h_logical = window::window_height(row_count, max_h_logical);
    let inner_w = (window::WINDOW_W_LOGICAL * scale).round() as i32;
    let inner_h = (new_h_logical as f64 * scale).round() as i32;

    let bottom_anchored = window::is_bottom_anchored(cur, area);
    let with_width = window::Rect::new(cur.x, cur.y, inner_w + frame_w, cur.h);
    let desired = window::reflow_height(with_width, inner_h + frame_h, bottom_anchored);
    let clamped = window::clamp_into(desired, area);

    let set_w = (clamped.w - frame_w).max(1) as u32;
    let set_h = (clamped.h - frame_h).max(1) as u32;
    let _ = win.set_size(tauri::PhysicalSize::new(set_w, set_h));
    let _ = win.set_position(tauri::PhysicalPosition::new(clamped.x, clamped.y));
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
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

/// Flip the header's grey toggle, persist it, and let `after_change`
/// repaint and resize off the now-different visible row count.
#[tauri::command]
fn toggle_hide_unknown(shared: State<Shared>, app: tauri::AppHandle) {
    let mut reg = shared.0.lock().unwrap();
    reg.hide_unknown = !reg.hide_unknown;
    let hide_unknown = reg.hide_unknown;
    let change = prepare_change(&reg);
    drop(reg);
    persist::save_hide_unknown(hide_unknown);
    after_change(&app, change);
}

/// Raise the host window of the session bound to a row. Returns a
/// sentence the surface shows as a brief inline row note either way;
/// Reveal never fails silently (docs/CONTROL_MAPPING.md).
#[tauri::command]
fn reveal_session(index: usize, shared: State<Shared>) -> String {
    let (label, cwd, dir, pid, session_id) = {
        let reg = shared.0.lock().unwrap();
        let Some(session) = reg.bindings.get(index).and_then(|id| reg.sessions.get(id)) else {
            return "No session is bound to this row.".to_string();
        };
        (
            session.label.clone(),
            session.cwd.clone(),
            session.cwd.as_deref().map(state::dir_name),
            session.pid,
            session.id.clone(),
        )
    };
    reveal::reveal(&label, dir.as_deref(), cwd.as_deref(), pid, &session_id)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            snapshot,
            select_tile,
            quit,
            reveal_session,
            toggle_hide_unknown
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window");

            // PR review P1: a saved position is only trusted if it still
            // intersects a monitor that is actually connected right now.
            // A nominal one-row size stands in for the real size, which
            // is not known until the bindings below are loaded.
            {
                let areas = all_work_areas(&window);
                let scale = window.scale_factor().unwrap_or(1.0);
                let nominal_w = (window::WINDOW_W_LOGICAL * scale).round() as i32;
                let nominal_h = (window::window_height(0, i32::MAX) as f64 * scale).round() as i32;
                let fallback_area = areas
                    .first()
                    .copied()
                    .unwrap_or_else(|| window::Rect::new(0, 0, nominal_w, nominal_h));
                let fallback = window::default_rect(fallback_area, nominal_w, nominal_h);
                let target = match persist::load_window_pos() {
                    Some(pos) => {
                        let saved = window::Rect::new(pos.x, pos.y, nominal_w, nominal_h);
                        window::restore_or_fallback(saved, &areas, fallback)
                    }
                    None => fallback,
                };
                let _ = window.set_position(tauri::PhysicalPosition::new(target.x, target.y));

                #[cfg(windows)]
                {
                    let hwnd = window.hwnd()?.0 as isize;
                    win_style::apply_noactivate(hwnd);
                }
            }

            let shared = Arc::new(Mutex::new(registry::Registry::default()));
            {
                let mut reg = shared.lock().unwrap();
                persist::load_bindings(&mut reg, now_ms());
                reg.hide_unknown = persist::load_hide_unknown();
            }
            app.manage(Shared(shared.clone()));
            let initial_rows = visible_rows(&shared.lock().unwrap());
            LAST_ROW_COUNT.store(initial_rows, Ordering::SeqCst);
            resize_for_rows(&window, initial_rows);

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
                            let change = prepare_change(&reg);
                            drop(reg);
                            after_change(&apply_handle, change);
                        }
                    }
                })
                .expect("spawn apply thread");

            // T_unknown watchdog. Never changes who is bound, but can
            // change who is visible: a session tipping into unknown
            // while the grey toggle is on must shrink the window the
            // same as a binding disappearing would, so this goes through
            // after_change like any other registry mutation rather than
            // emitting a snapshot on its own.
            let tick_handle = app.handle().clone();
            let tick_reg = shared.clone();
            std::thread::Builder::new()
                .name("deckhand-tick".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(2));
                    let mut reg = tick_reg.lock().unwrap();
                    if reg.tick(now_ms()) {
                        let change = prepare_change(&reg);
                        drop(reg);
                        after_change(&tick_handle, change);
                    }
                })
                .expect("spawn tick thread");

            // Cold start, then a periodic rescan every RESCAN_INTERVAL:
            // `claude agents` shells out, so it always runs outside the
            // registry lock; only applying the result takes it. States
            // stay unknown until events arrive (ADR-024); a failed run
            // (claude missing, unparseable output) changes nothing,
            // including pruning nothing, since fetch's `None` carries no
            // information about who is still alive.
            let scan_handle = app.handle().clone();
            let scan_reg = shared.clone();
            std::thread::Builder::new()
                .name("deckhand-scan".into())
                .spawn(move || loop {
                    if let Some(rows) = enumerate::fetch() {
                        let mut reg = scan_reg.lock().unwrap();
                        if enumerate::register(&mut reg, &rows, now_ms()) {
                            let change = prepare_change(&reg);
                            drop(reg);
                            after_change(&scan_handle, change);
                        }
                    }
                    std::thread::sleep(RESCAN_INTERVAL);
                })
                .expect("spawn scan thread");

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| match event {
            tauri::RunEvent::Exit => {
                persist::remove_daemon_contact();
            }
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::Moved(pos),
                ..
            } => {
                persist::save_window_pos(pos.x, pos.y);
            }
            _ => {}
        });
}
