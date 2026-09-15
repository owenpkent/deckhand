// Crash restart (ADR-037): Deckhand relaunches itself after a crash,
// bounded by a short backoff so a stuck configuration cannot restart
// forever and spin the machine. Split in two:
//
// - A pure, platform-independent policy half (`decide`, plus the
//   ledger read/append) that is testable on every platform without a
//   real process or a real clock behind it.
// - A Windows-only runtime half (`spawn_for`, `run`, `wait_exit_code`)
//   that actually spawns and waits on processes. The watchdog is this
//   same binary, launched by itself into a headless mode
//   (`--watchdog <parent_pid>`, wired in `main.rs`), not a separate
//   executable.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The sliding window a restart must fall inside to count against the
/// cap: ten minutes.
pub const RESTART_WINDOW_MS: i64 = 600_000;

/// Restarts allowed inside `RESTART_WINDOW_MS` before the watchdog gives
/// up rather than restart yet again.
pub const MAX_RESTARTS_IN_WINDOW: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Exit code 0: a Quit, or any other normal shutdown. Never
    /// restarted.
    CleanExit,
    Restart,
    GaveUp,
}

/// Decide what to do about a process that just exited, from its exit
/// code and the restart timestamps already on record. Pure: no I/O, no
/// clock read, so every case is a plain unit test and `run` below is
/// the only thing that has to supply real inputs.
pub fn decide(exit_code: u32, history_ms: &[i64], now_ms: i64) -> Decision {
    if exit_code == 0 {
        return Decision::CleanExit;
    }
    let recent_restarts = history_ms.iter().filter(|&&t| now_ms - t < RESTART_WINDOW_MS).count();
    if recent_restarts >= MAX_RESTARTS_IN_WINDOW {
        Decision::GaveUp
    } else {
        Decision::Restart
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The ledger's path under `dir` (persist.rs's `data_dir()` in real
/// use, a throwaway directory in tests, following persist.rs's own
/// `_in(dir)` pattern).
pub fn watchdog_log_path(dir: &Path) -> PathBuf {
    dir.join("watchdog.log")
}

/// Parse the timestamps of every well-formed `restart` line in the
/// ledger, oldest and newest alike; a `gave-up` line (or anything else
/// that does not parse as `"<ms> <exit code> restart"`) is skipped
/// rather than failing the whole read. A damaged or foreign-format
/// ledger must never stop the watchdog from deciding at all, only from
/// remembering perfectly.
pub fn read_restarts_in(dir: &Path) -> Vec<i64> {
    let Ok(body) = std::fs::read_to_string(watchdog_log_path(dir)) else {
        return Vec::new();
    };
    body.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let ts = parts.next()?.parse::<i64>().ok()?;
            let _exit_code = parts.next()?;
            let tag = parts.next()?;
            if parts.next().is_some() {
                return None; // an extra field: not a line this build wrote
            }
            (tag == "restart").then_some(ts)
        })
        .collect()
}

/// Append one line to the ledger, creating the directory and file as
/// needed. A `CleanExit` writes nothing: the ledger exists only to
/// bound crash restarts, and a clean exit neither counts toward that
/// bound nor needs to be read back later. Every failure here (a
/// directory that would not create, a locked file, anything else) is
/// swallowed rather than surfaced: this runs from the watchdog's own
/// headless main, where there is nobody to show an error to, and
/// panicking here would defeat the entire point of a crash-recovery
/// path.
pub fn append_in(dir: &Path, now_ms: i64, exit_code: u32, decision: &Decision) {
    let tag = match decision {
        Decision::CleanExit => return,
        Decision::Restart => "restart",
        Decision::GaveUp => "gave-up",
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let line = format!("{now_ms} {exit_code} {tag}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(watchdog_log_path(dir)) {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(windows)]
mod win {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, WaitForSingleObject, CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW,
        INFINITE, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    use super::Decision;

    /// Spawn this same exe in watchdog mode, detached from the console
    /// and, where the job this process is in allows it, from that job
    /// too (`CREATE_BREAKAWAY_FROM_JOB`), so the watchdog survives
    /// whatever takes its parent down. A job that forbids breakaway
    /// rejects the spawn outright; the retry drops that flag and keeps
    /// only `CREATE_NO_WINDOW`.
    pub fn spawn_for(parent_pid: u32) -> std::io::Result<()> {
        let exe = std::env::current_exe()?;
        let pid_arg = parent_pid.to_string();
        let try_spawn = |flags: u32| {
            Command::new(&exe)
                .args(["--watchdog", &pid_arg])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(flags)
                .spawn()
        };
        match try_spawn(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB) {
            Ok(_) => Ok(()),
            Err(_) => try_spawn(CREATE_NO_WINDOW).map(|_| ()),
        }
    }

    /// Open a handle on `pid` and block until it exits, returning its
    /// exit code. `None` when the pid cannot even be opened (already
    /// gone, or access denied) -- treated by `run` as nothing to
    /// restart, never as a guess at what happened.
    pub fn wait_exit_code(pid: u32) -> Option<u32> {
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            WaitForSingleObject(handle, INFINITE);
            let mut code: u32 = 0;
            let got_code = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            got_code.then_some(code)
        }
    }

    /// The watchdog's whole main: wait for the parent to exit, decide
    /// whether to relaunch it against the ledger, record the decision,
    /// and relaunch when the decision says to. Always returns 0; this
    /// process's own exit code is never inspected by anything.
    pub fn run(parent_pid: u32) -> i32 {
        let Some(exit_code) = wait_exit_code(parent_pid) else {
            return 0;
        };
        let now = super::now_ms();
        let dir = crate::persist::data_dir();
        let history = dir.as_deref().map(super::read_restarts_in).unwrap_or_default();
        let decision = super::decide(exit_code, &history, now);
        if let Some(dir) = dir.as_deref() {
            super::append_in(dir, now, exit_code, &decision);
        }
        if matches!(decision, Decision::Restart) {
            relaunch();
        }
        0
    }

    /// Relaunch the app itself, detached the same way `spawn_for`
    /// detaches the watchdog, minus `CREATE_NO_WINDOW`: this is the
    /// ordinary GUI relaunch, not a background console tool, so it must
    /// not be created hidden. `DETACHED_PROCESS` is not needed:
    /// breaking away from the job (or, on the fallback, nothing at all)
    /// is enough to outlive this watchdog process.
    fn relaunch() {
        let Ok(exe) = std::env::current_exe() else { return };
        let try_spawn = |flags: u32| {
            Command::new(&exe)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(flags)
                .spawn()
        };
        if try_spawn(CREATE_BREAKAWAY_FROM_JOB).is_err() {
            let _ = try_spawn(0);
        }
    }
}

#[cfg(windows)]
pub use win::{run, spawn_for, wait_exit_code};

#[cfg(not(windows))]
pub fn spawn_for(_parent_pid: u32) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn run(_parent_pid: u32) -> i32 {
    0
}

#[cfg(not(windows))]
pub fn wait_exit_code(_pid: u32) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    // A fresh, unique directory per test, mirroring persist.rs's own
    // temp_dir helper: never the real data_dir(), never shared across
    // tests running in parallel.
    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("deckhand-watchdog-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    // ---- decide --------------------------------------------------------

    #[test]
    fn exit_code_zero_is_always_clean_exit() {
        assert_eq!(decide(0, &[], 1_000_000), Decision::CleanExit);
        assert_eq!(decide(0, &[999_900, 999_950], 1_000_000), Decision::CleanExit);
    }

    #[test]
    fn a_first_crash_with_no_history_restarts() {
        assert_eq!(decide(1, &[], 1_000_000), Decision::Restart);
    }

    #[test]
    fn a_third_crash_inside_the_window_gives_up() {
        let now = 1_000_000;
        // Three prior restarts, all recent: this would be the fourth.
        let history = [now - 1, now - 2, now - 3];
        assert_eq!(decide(1, &history, now), Decision::GaveUp);
    }

    #[test]
    fn a_second_crash_inside_the_window_still_restarts() {
        let now = 1_000_000;
        let history = [now - 1, now - 2];
        assert_eq!(decide(1, &history, now), Decision::Restart);
    }

    #[test]
    fn history_outside_the_window_does_not_count_and_still_restarts() {
        let now = 1_000_000;
        // Three restarts, but all well outside the ten-minute window.
        let history = [now - RESTART_WINDOW_MS - 1, now - RESTART_WINDOW_MS - 2, now - RESTART_WINDOW_MS - 3];
        assert_eq!(decide(1, &history, now), Decision::Restart);
    }

    // ---- ledger ----------------------------------------------------------

    #[test]
    fn the_ledger_round_trips_restart_timestamps() {
        let dir = temp_dir();
        append_in(&dir, 100, 1, &Decision::Restart);
        append_in(&dir, 200, 1, &Decision::Restart);
        assert_eq!(read_restarts_in(&dir), vec![100, 200]);
        cleanup(&dir);
    }

    #[test]
    fn gave_up_lines_are_written_but_not_counted_as_restarts() {
        let dir = temp_dir();
        append_in(&dir, 100, 1, &Decision::Restart);
        append_in(&dir, 200, 1, &Decision::GaveUp);
        assert_eq!(read_restarts_in(&dir), vec![100]);
        let body = std::fs::read_to_string(watchdog_log_path(&dir)).unwrap();
        assert!(body.contains("200 1 gave-up"));
        cleanup(&dir);
    }

    #[test]
    fn a_clean_exit_writes_nothing() {
        let dir = temp_dir();
        append_in(&dir, 100, 0, &Decision::CleanExit);
        assert!(!watchdog_log_path(&dir).exists());
        cleanup(&dir);
    }

    #[test]
    fn reading_a_ledger_that_does_not_exist_gives_an_empty_history() {
        let dir = temp_dir();
        assert!(read_restarts_in(&dir).is_empty());
        cleanup(&dir);
    }

    #[test]
    fn malformed_lines_are_skipped_rather_than_failing_the_whole_read() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let body = "100 1 restart\nnot a line\n200 restart\n300 1 restart extra\n400 1 restart\n";
        std::fs::write(watchdog_log_path(&dir), body).unwrap();
        assert_eq!(read_restarts_in(&dir), vec![100, 400]);
        cleanup(&dir);
    }

    #[test]
    fn append_creates_the_directory_when_missing() {
        let dir = temp_dir().join("nested").join("deeper");
        append_in(&dir, 1, 1, &Decision::Restart);
        assert_eq!(read_restarts_in(&dir), vec![1]);
        cleanup(&dir);
    }
}

#[cfg(all(test, windows))]
mod win_tests {
    use super::*;

    #[test]
    fn wait_exit_code_opens_and_waits_itself_rather_than_relying_on_child_wait() {
        let mut child = std::process::Command::new("cmd").args(["/c", "exit", "7"]).spawn().expect("spawn cmd");
        // Deliberately not calling child.wait() first: wait_exit_code
        // must open its own handle and block on it itself.
        let code = wait_exit_code(child.id());
        assert_eq!(code, Some(7));
        let _ = child.wait();
    }
}
