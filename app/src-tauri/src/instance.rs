// Single instance (ADR-037): only one Deckhand may run at a time. A
// second launch must discover the first is already running and step
// aside, rather than starting a second daemon that binds a second
// loopback port and leaves the hook shim talking to whichever one
// happens to answer.
//
// The mechanism is a named kernel mutex, `Local\Deckhand.Instance`.
// `CreateMutexW` either creates a new mutex object or opens the
// existing one of the same name; `GetLastError` is how the two
// outcomes are told apart (`ERROR_ALREADY_EXISTS`). The handle this
// returns is deliberately never closed: leaking it lets the kernel hold
// the mutex for the whole life of this process and release it
// automatically on any exit, including a crash, so there is no cleanup
// path here to get wrong or skip.

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::CreateMutexW;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
};

/// The name every real launch of Deckhand claims. Tests use a distinct
/// name (`claim_named`) so a test run can never collide with, or be
/// confused for, an actual running Deckhand.
const INSTANCE_MUTEX_NAME: &str = "Local\\Deckhand.Instance";

/// `tauri.conf.json`'s main window title, which `raise_existing` looks
/// for with `FindWindowW`.
#[cfg(windows)]
const WINDOW_TITLE: &str = "Deckhand";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    First,
    AlreadyRunning,
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Claim a named single-instance mutex. `First` on the actual first
/// claim, or on any failure to create the mutex at all: a mutex API
/// failure must never block the app from starting, so it is treated the
/// same as nobody else running rather than surfaced as an error with
/// nothing to show it to.
#[cfg(windows)]
pub fn claim_named(name: &str) -> Claim {
    let wide_name = wide(name);
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
    if handle.is_null() {
        return Claim::First;
    }
    // `handle` is intentionally left open for the rest of the process's
    // life; see the module doc.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        Claim::AlreadyRunning
    } else {
        Claim::First
    }
}

#[cfg(not(windows))]
pub fn claim_named(_name: &str) -> Claim {
    Claim::First
}

/// Claim the real, single-process-lifetime instance mutex. Called once,
/// at the very top of `main`.
pub fn claim() -> Claim {
    claim_named(INSTANCE_MUTEX_NAME)
}

/// Bring the already-running Deckhand's window to the top of the
/// z-order without activating it: the window is a no-focus-steal
/// surface (see `win_style::apply_noactivate` in `main.rs`), and a
/// second launch raising the first must not steal focus from whatever
/// the owner was doing any more than the first launch itself would
/// have. A missing window (found no match) is silently a no-op: there
/// is nothing else this process can usefully do before exiting anyway.
#[cfg(windows)]
pub fn raise_existing() {
    let title = wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

#[cfg(not(windows))]
pub fn raise_existing() {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn claiming_the_same_name_twice_in_one_process_is_first_then_already_running() {
        // A test-only mutex name, distinct from `INSTANCE_MUTEX_NAME`,
        // so this test can never collide with a real, running
        // Deckhand's own mutex.
        let name = "Local\\Deckhand.Instance.Test";
        assert_eq!(claim_named(name), Claim::First);
        assert_eq!(claim_named(name), Claim::AlreadyRunning);
    }
}
