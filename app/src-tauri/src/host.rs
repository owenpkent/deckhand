// Host classification for Reveal (docs/CONTROL_MAPPING.md). A session's
// pid alone cannot be matched against a top-level window on every host:
// a VS Code extension host and a Windows Terminal tab shell both sit a
// few processes below the window owner. Classifying the host first lets
// reveal.rs pick a targeting strategy that actually fits each one.
//
// `classify` is plain data in, data out: a process table and a starting
// pid, nothing that touches Win32, so it is unit-tested without a real
// process tree. `snapshot_processes` (cfg(windows)) is the thin
// Toolhelp32 wrapper that builds the table classify reads.

use std::collections::{HashMap, HashSet};

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// A process id's chain is walked at most this many hops before giving
/// up and calling it a plain console: past this depth the exe search is
/// more likely to be a cycle or a pathological tree than a real answer.
const MAX_HOPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    VsCode,
    WindowsTerminal,
    Console,
}

/// Walk `pid`'s parent chain in `procs` (pid -> (parent pid, exe name)),
/// stopping at the first ancestor whose exe names a host Reveal treats
/// specially. Cycle-safe: a pid seen twice (PID reuse can genuinely
/// produce a loop) ends the walk rather than spinning forever. Anything
/// unrecognised, including a pid missing from the table, falls back to
/// `Console`, the 1:1 pid-to-window host.
pub fn classify(pid: u32, procs: &HashMap<u32, (u32, String)>) -> Host {
    let mut seen = HashSet::new();
    let mut current = pid;
    for _ in 0..MAX_HOPS {
        if !seen.insert(current) {
            break;
        }
        let Some((parent, exe)) = procs.get(&current) else {
            break;
        };
        if is_vscode_exe(exe) {
            return Host::VsCode;
        }
        if exe.eq_ignore_ascii_case("windowsterminal.exe") {
            return Host::WindowsTerminal;
        }
        current = *parent;
    }
    Host::Console
}

/// True for the stable and Insiders builds; case-insensitive since a
/// process table's casing is whatever the OS happened to record.
pub fn is_vscode_exe(exe: &str) -> bool {
    exe.eq_ignore_ascii_case("Code.exe") || exe.eq_ignore_ascii_case("Code - Insiders.exe")
}

/// The same walk as `classify`, but naming the winning ancestor's own
/// pid instead of which host it is. Meaningful once `classify` has
/// already said `Host::VsCode` for this pid: that walk and this one
/// check the same predicate in the same order, so this is guaranteed to
/// find the same process. Reveal's VS Code CLI lookup
/// (`reveal::reveal_vscode`) needs the pid itself, not just the
/// classification, to ask Windows for that process's image path: the
/// session's own pid belongs to claude.exe, a few hops below the window
/// owner.
pub fn vscode_ancestor_pid(pid: u32, procs: &HashMap<u32, (u32, String)>) -> Option<u32> {
    let mut seen = HashSet::new();
    let mut current = pid;
    for _ in 0..MAX_HOPS {
        if !seen.insert(current) {
            break;
        }
        let Some((parent, exe)) = procs.get(&current) else {
            break;
        };
        if is_vscode_exe(exe) {
            return Some(current);
        }
        current = *parent;
    }
    None
}

/// True when `pid`'s *immediate* parent (one hop, not a walk) is a VS
/// Code process. This is the shape of a session opened straight from
/// the extension host; a session running inside VS Code's own
/// integrated terminal sits several hops further down (claude.exe ->
/// shell -> conpty/OpenConsole -> ... -> Code.exe) and must read false
/// here. Reveal's VS Code session-tab link is only ever safe to fire
/// for the former: opening it for a terminal session would spawn a
/// second, duplicate `claude --resume` process rather than reveal the
/// existing one (docs/CONTROL_MAPPING.md; see reveal.rs::reveal_vscode).
pub fn parent_is_vscode_exe(pid: u32, procs: &HashMap<u32, (u32, String)>) -> bool {
    procs
        .get(&pid)
        .and_then(|(parent, _)| procs.get(parent))
        .map(|(_, exe)| is_vscode_exe(exe))
        .unwrap_or(false)
}

/// Build the pid -> (parent pid, exe name) table `classify` walks, from
/// a Toolhelp32 snapshot of every process on the system. Returns an
/// empty table on any failure; `classify` already treats a pid missing
/// from the table as an unclassifiable `Console`, so callers need no
/// separate error path.
#[cfg(windows)]
pub fn snapshot_processes() -> HashMap<u32, (u32, String)> {
    let mut procs = HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return procs;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
                procs.insert(entry.th32ProcessID, (entry.th32ParentProcessID, exe));
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    procs
}

/// Both facts registry.rs needs about a pid to decide VS Code
/// supersession (docs/DECISIONS.md, 2026-09-15's four-session bug): its
/// host classification, and its own *immediate* OS parent pid -- not
/// walked any further up, unlike `classify` and `vscode_ancestor_pid`,
/// which both climb to find a Code.exe ancestor however many hops away.
/// One Toolhelp32 snapshot answers both, taken once when a session's
/// pid is first learned or replaced (registry.rs `register_enumerated`),
/// never re-walked on every tick.
#[cfg(windows)]
pub fn resolve(pid: u32) -> (Host, Option<u32>) {
    let procs = snapshot_processes();
    let host = classify(pid, &procs);
    let parent_pid = procs.get(&pid).map(|(parent, _)| *parent);
    (host, parent_pid)
}

/// The non-Windows stub: no process table exists to classify from, so
/// every pid reads as a plain console with no known parent, the same
/// "nothing special observed" default `classify` itself falls back to
/// for a pid missing from its own table.
#[cfg(not(windows))]
pub fn resolve(_pid: u32) -> (Host, Option<u32>) {
    (Host::Console, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(entries: &[(u32, u32, &str)]) -> HashMap<u32, (u32, String)> {
        entries.iter().map(|(pid, parent, exe)| (*pid, (*parent, exe.to_string()))).collect()
    }

    #[test]
    fn a_claude_exe_under_an_extension_host_under_main_code_is_vscode() {
        // claude.exe (200) -> extension host Code.exe (100) -> main
        // Code.exe (1) -> explorer (0), the shape described on the
        // owner's machine.
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "Code.exe"), (1, 0, "explorer.exe")]);
        assert_eq!(classify(200, &procs), Host::VsCode);
    }

    #[test]
    fn vscode_insiders_is_also_recognised() {
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "Code - Insiders.exe")]);
        assert_eq!(classify(200, &procs), Host::VsCode);
    }

    #[test]
    fn a_tab_shell_under_windows_terminal_is_windows_terminal() {
        // claude.exe (200) -> a tab's conhost/OpenConsole (150) -> the
        // one WindowsTerminal.exe process (100) that owns every window.
        let procs = table(&[(200, 150, "claude.exe"), (150, 100, "OpenConsole.exe"), (100, 1, "WindowsTerminal.exe")]);
        assert_eq!(classify(200, &procs), Host::WindowsTerminal);
    }

    #[test]
    fn exe_matching_is_case_insensitive() {
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "CODE.EXE")]);
        assert_eq!(classify(200, &procs), Host::VsCode);
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "windowsterminal.exe")]);
        assert_eq!(classify(200, &procs), Host::WindowsTerminal);
    }

    #[test]
    fn a_bare_console_process_with_no_special_ancestor_is_console() {
        let procs = table(&[(200, 1, "cmd.exe"), (1, 0, "explorer.exe")]);
        assert_eq!(classify(200, &procs), Host::Console);
    }

    #[test]
    fn a_pid_missing_from_the_table_is_console() {
        let procs = HashMap::new();
        assert_eq!(classify(999, &procs), Host::Console);
    }

    #[test]
    fn a_cycle_in_the_parent_chain_terminates_and_is_console() {
        // 1 -> 2 -> 1 -> ...: PID reuse can genuinely produce this.
        let procs = table(&[(1, 2, "a.exe"), (2, 1, "b.exe")]);
        assert_eq!(classify(1, &procs), Host::Console);
    }

    #[test]
    fn a_chain_longer_than_max_hops_gives_up_and_is_console() {
        // 0..=9 each parented by the next; the special exe sits at hop
        // 9, past the 8-hop budget, so it must never be found.
        let mut entries: Vec<(u32, u32, String)> = (0..9).map(|i| (i, i + 1, "chain.exe".to_string())).collect();
        entries.push((9, 10, "Code.exe".to_string()));
        let procs: HashMap<u32, (u32, String)> =
            entries.into_iter().map(|(pid, parent, exe)| (pid, (parent, exe))).collect();
        assert_eq!(classify(0, &procs), Host::Console);
    }

    #[test]
    fn the_starting_pid_itself_is_checked_before_walking_up() {
        // claude.exe never matches, but this pins that `classify` checks
        // the given pid's own entry, not only its ancestors, before
        // advancing to the parent.
        let procs = table(&[(200, 100, "Code.exe"), (100, 1, "explorer.exe")]);
        assert_eq!(classify(200, &procs), Host::VsCode);
    }

    // ---- vscode_ancestor_pid --------------------------------------------

    #[test]
    fn vscode_ancestor_pid_names_the_extension_host_not_the_main_process() {
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "Code.exe"), (1, 0, "explorer.exe")]);
        assert_eq!(vscode_ancestor_pid(200, &procs), Some(100));
    }

    #[test]
    fn vscode_ancestor_pid_is_none_off_a_plain_console() {
        let procs = table(&[(200, 1, "cmd.exe"), (1, 0, "explorer.exe")]);
        assert_eq!(vscode_ancestor_pid(200, &procs), None);
    }

    #[test]
    fn vscode_ancestor_pid_gives_up_past_max_hops_like_classify_does() {
        let mut entries: Vec<(u32, u32, String)> = (0..9).map(|i| (i, i + 1, "chain.exe".to_string())).collect();
        entries.push((9, 10, "Code.exe".to_string()));
        let procs: HashMap<u32, (u32, String)> =
            entries.into_iter().map(|(pid, parent, exe)| (pid, (parent, exe))).collect();
        assert_eq!(vscode_ancestor_pid(0, &procs), None);
    }

    // ---- parent_is_vscode_exe --------------------------------------------

    #[test]
    fn an_extension_hosted_session_has_code_exe_as_its_immediate_parent() {
        let procs = table(&[(200, 100, "claude.exe"), (100, 1, "Code.exe")]);
        assert!(parent_is_vscode_exe(200, &procs));
    }

    #[test]
    fn a_session_in_the_integrated_terminal_has_a_shell_as_its_immediate_parent() {
        // claude.exe (200) -> powershell (150) -> ... -> Code.exe (1):
        // Code.exe is an ancestor, but not the *immediate* parent, so
        // this must read false even though `classify` says VsCode.
        let procs = table(&[(200, 150, "claude.exe"), (150, 1, "pwsh.exe"), (1, 0, "Code.exe")]);
        assert!(!parent_is_vscode_exe(200, &procs));
        assert_eq!(classify(200, &procs), Host::VsCode, "sanity: classify still finds Code.exe up the chain");
    }

    #[test]
    fn parent_is_vscode_exe_is_false_when_the_pid_or_its_parent_is_missing() {
        let procs = table(&[(200, 100, "claude.exe")]);
        assert!(!parent_is_vscode_exe(200, &procs), "parent 100 is missing from the table");
        assert!(!parent_is_vscode_exe(999, &procs), "999 itself is missing from the table");
    }

    // ---- resolve ---------------------------------------------------------

    #[cfg(windows)]
    #[test]
    fn resolve_finds_this_processs_own_parent_pid_in_a_real_snapshot() {
        let (_, parent) = resolve(std::process::id());
        assert!(parent.is_some(), "this running process must appear in its own process table with a parent");
    }

    #[cfg(not(windows))]
    #[test]
    fn resolve_is_a_console_with_no_parent_off_the_non_windows_stub() {
        assert_eq!(resolve(4242), (Host::Console, None));
    }
}
