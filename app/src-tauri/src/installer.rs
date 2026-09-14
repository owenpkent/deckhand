// The settings panel's Repair action: reruns scripts/install-hooks.ps1
// against the running exe's own checkout (docs/DECISIONS.md#adr-033).
// This is a Phase 1 dev assumption, not a real installer story
// (TODO.md, Backlog): it only works when the exe is running out of a
// git checkout that still has its scripts/ folder alongside it. Finding
// that checkout is a pure filesystem walk, unit-tested with a throwaway
// directory tree; actually running the script is the impure edge,
// gated `#[cfg(windows)]` like the rest of the Win32-specific code in
// this daemon.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// How far up from the running exe to look before giving up. Generous
/// enough to cover both `target/debug/deckhand.exe` (two levels under
/// the repo root) and a deeper build layout, without ever being so deep
/// that an unrelated ancestor directory (the user's whole home folder,
/// say) could spuriously match.
const MAX_ANCESTORS: u8 = 8;

/// The installer this panel can run is scoped to whichever checkout the
/// running exe happens to sit inside, found by walking up looking for
/// the one file the checkout needs: `scripts/install-hooks.ps1`. Returns
/// `None` when no ancestor within `MAX_ANCESTORS` levels has one, which
/// is the ordinary shape of an installed (not a dev) build, and is what
/// disables the Repair row rather than erroring.
pub fn find_repo_root(exe_path: &Path) -> Option<PathBuf> {
    let mut dir = exe_path.parent();
    for _ in 0..MAX_ANCESTORS {
        let d = dir?;
        if d.join("scripts").join("install-hooks.ps1").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Repair is bounded so a hung installer (a wedged PowerShell process, a
/// prompt it never expected to hit) cannot hang the settings panel
/// forever; the command it backs is `async` precisely so this wait never
/// blocks the webview/event thread either way (main.rs, matching
/// `activate_session`'s own spawn_blocking pattern).
pub const REPAIR_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairOutcome {
    /// The installer process ran to completion (any exit code; its own
    /// output is not inspected, only whether it finished). The caller
    /// re-reads the hook status afterward to see whether it actually
    /// fixed anything.
    Ran,
    /// Still running past `REPAIR_TIMEOUT`; killed rather than left to
    /// finish unattended.
    TimedOut,
    /// `powershell.exe` itself never started (not on PATH, or some
    /// other launch failure).
    FailedToStart,
}

#[cfg(windows)]
mod run {
    use super::{RepairOutcome, REPAIR_TIMEOUT};
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;
    use std::time::Instant;

    /// `CREATE_NO_WINDOW` (winbase.h, `0x0800_0000`): Repair is a
    /// background action triggered from the settings panel, not a
    /// script the owner is watching run, so no console window should
    /// flash up for it. Hardcoded rather than pulled from `windows-sys`
    /// the same way `main.rs`'s `win_style` module hardcodes its own
    /// Win32 constants.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Runs `scripts\install-hooks.ps1` under `repo_root` hidden, with
    /// no window and no arguments beyond the script path itself (the
    /// caller may want `-ShimPath`, but the running exe's own shim
    /// sibling is exactly what install-hooks.ps1 already defaults to,
    /// so nothing needs passing). Blocking: the caller is expected to
    /// run this inside `spawn_blocking`.
    pub fn run_repair(repo_root: &Path) -> RepairOutcome {
        let script = repo_root.join("scripts").join("install-hooks.ps1");
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
        cmd.arg(&script);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return RepairOutcome::FailedToStart,
        };
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => return RepairOutcome::Ran,
                Ok(None) => {
                    if start.elapsed() >= REPAIR_TIMEOUT {
                        let _ = child.kill();
                        let _ = child.wait();
                        return RepairOutcome::TimedOut;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => return RepairOutcome::FailedToStart,
            }
        }
    }
}

#[cfg(windows)]
pub use run::run_repair;

/// Every other platform never finds a repo root either (`find_repo_root`
/// still runs, it just will not find `install-hooks.ps1` there), so
/// this is unreachable in practice; it exists only so the crate compiles
/// off Windows.
#[cfg(not(windows))]
pub fn run_repair(_repo_root: &Path) -> RepairOutcome {
    RepairOutcome::FailedToStart
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("deckhand-installer-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finds_the_repo_root_two_levels_above_target_debug() {
        let repo = temp_dir();
        fs::create_dir_all(repo.join("scripts")).unwrap();
        fs::write(repo.join("scripts").join("install-hooks.ps1"), "").unwrap();
        let exe_dir = repo.join("target").join("debug");
        fs::create_dir_all(&exe_dir).unwrap();
        let exe = exe_dir.join("deckhand.exe");

        assert_eq!(find_repo_root(&exe), Some(repo.clone()));
        cleanup(&repo);
    }

    #[test]
    fn finds_the_repo_root_when_the_exe_sits_directly_in_it() {
        let repo = temp_dir();
        fs::create_dir_all(repo.join("scripts")).unwrap();
        fs::write(repo.join("scripts").join("install-hooks.ps1"), "").unwrap();
        let exe = repo.join("deckhand.exe");

        assert_eq!(find_repo_root(&exe), Some(repo.clone()));
        cleanup(&repo);
    }

    #[test]
    fn returns_none_when_no_ancestor_has_the_installer() {
        let dir = temp_dir();
        let exe_dir = dir.join("a").join("b").join("c");
        fs::create_dir_all(&exe_dir).unwrap();
        let exe = exe_dir.join("deckhand.exe");

        assert_eq!(find_repo_root(&exe), None);
        cleanup(&dir);
    }

    // Pins the wire shape app/ui/src/types.ts's RepairOutcome mirrors.
    #[test]
    fn serializes_as_lowercase_snake_case() {
        assert_eq!(serde_json::to_string(&RepairOutcome::Ran).unwrap(), "\"ran\"");
        assert_eq!(serde_json::to_string(&RepairOutcome::TimedOut).unwrap(), "\"timed_out\"");
        assert_eq!(serde_json::to_string(&RepairOutcome::FailedToStart).unwrap(), "\"failed_to_start\"");
    }

    #[test]
    fn does_not_walk_past_max_ancestors() {
        // Ten levels deep with the installer only at the very top: one
        // level beyond MAX_ANCESTORS's reach from the exe's own
        // directory, so this must not find it.
        let dir = temp_dir();
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("scripts").join("install-hooks.ps1"), "").unwrap();
        let mut exe_dir = dir.clone();
        for i in 0..(MAX_ANCESTORS as usize + 2) {
            exe_dir = exe_dir.join(format!("d{i}"));
        }
        fs::create_dir_all(&exe_dir).unwrap();
        let exe = exe_dir.join("deckhand.exe");

        assert_eq!(find_repo_root(&exe), None);
        cleanup(&dir);
    }
}
