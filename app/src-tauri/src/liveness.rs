// Process liveness (ADR-035): whether the OS process behind a session's
// pid is still running, independent of what colour the session shows.
// This is deliberately narrow, one bit ("has it exited"), because
// that one bit is all `registry.rs` needs for two things: trusting a
// session a scan omits (`prune_missing`, a held handle beats a scan
// that missed it) and declaring a session's process gone even with no
// `SessionEnd` (`state.rs::process_exited`, via `Registry::tick`).
// Everything about *what colour* a session shows still comes from hooks
// and the scan's own status string (state.rs), never from here.

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

/// Whether the OS process a `Watch` was opened for has exited.
pub trait Liveness: Send {
    fn exited(&self) -> bool;
}

pub type Watch = Box<dyn Liveness>;

#[cfg(windows)]
struct ProcessWatch {
    handle: HANDLE,
}

// A process handle is valid on any thread: nothing about it is
// thread-affine the way a window or COM handle can be.
#[cfg(windows)]
unsafe impl Send for ProcessWatch {}

#[cfg(windows)]
impl Liveness for ProcessWatch {
    fn exited(&self) -> bool {
        // A zero timeout: this only ever polls the handle's current
        // signalled state, never blocks. Signalled is exactly what a
        // process handle becomes on exit, and only on exit.
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
    }
}

#[cfg(windows)]
impl Drop for ProcessWatch {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Open a watch on `pid`'s process. `None` when the pid does not name a
/// process this daemon can open (already exited, or access denied).
///
/// Holding the handle open for as long as the watch lives also pins the
/// pid to this one process: Windows will not hand the same pid to a
/// different process while any handle to the original one remains open,
/// so there is no separate start-time check needed to rule out pid
/// reuse: the open handle already rules it out.
#[cfg(windows)]
pub fn open(pid: u32) -> Option<Watch> {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return None;
    }
    Some(Box::new(ProcessWatch { handle }))
}

/// The non-Windows stub: no platform liveness signal exists here, so
/// every session behaves as though it has no watch (the pre-ADR-035
/// behaviour, gated on scan sightings and hook silence alone).
#[cfg(not(windows))]
pub fn open(_pid: u32) -> Option<Watch> {
    None
}

/// A `Liveness` the registry's own tests can flip by hand, without a
/// real OS process behind it.
#[cfg(test)]
pub struct Fake(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

#[cfg(test)]
impl Liveness for Fake {
    fn exited(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_watch_on_this_process_is_not_exited() {
        let watch = open(std::process::id()).expect("this process's own pid must open");
        assert!(!watch.exited());
    }

    #[test]
    fn a_watch_on_an_exited_child_reports_exited() {
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "exit", "0"])
            .spawn()
            .expect("spawn cmd");
        let watch = open(child.id()).expect("a live child's pid must open");
        assert!(!watch.exited(), "the child has not been waited on yet");
        child.wait().expect("wait for the child to exit");
        // Child::wait already blocks until the OS has torn the process
        // down, so the handle's own wait is immediately signalled with
        // no extra delay needed here.
        assert!(watch.exited());
    }
}
