// Reveal: raise a session's host window (docs/CONTROL_MAPPING.md).
//
// A session's pid does not mean the same thing on every host
// (host.rs::classify, observed on this machine):
//   - Console (plain cmd/PowerShell): the session's own process owns its
//     own console window 1:1. AttachConsole gives an exact hwnd; no
//     title guessing needed.
//   - Windows Terminal: one WindowsTerminal.exe process owns every tab
//     across every window, and a tab's shell hangs off OpenConsole, not
//     off the window owner. Pid can only narrow "is this a Terminal
//     window at all"; picking among more than one is an honest miss.
//   - VS Code: every window (and every claude.exe running inside one) is
//     a child of one main Code.exe process by way of a per-window
//     extension host, so pid can only narrow "is this a VS Code window
//     at all" too; disambiguating further is a title match today and a
//     `code <folder>` / ~/.claude/ide/*.lock lookup in a later pass
//     (`reveal_vscode`).
//   - Unknown pid: the original scored title/pid match, unchanged.
//
// Scoring (`pick`) and the own-process filter (`exclude_own_process`)
// are plain data in, data out, so both are tested without a window
// manager. Everything that actually touches Win32 is cfg-gated per item
// instead of at module scope, so `Candidate`, `pick`, and
// `exclude_own_process` build and test on any host.

#[cfg(windows)]
use std::collections::HashMap;
#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
#[cfg(windows)]
use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, GetConsoleWindow};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, SW_RESTORE,
};

#[cfg(windows)]
use crate::host::{self, Host};

pub struct Candidate {
    pub hwnd: isize,
    pub title: String,
    pub pid: u32,
}

#[cfg(windows)]
extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len == 0 {
            return 1;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let list = &mut *(lparam as *mut Vec<Candidate>);
        list.push(Candidate {
            hwnd: hwnd as isize,
            title: String::from_utf16_lossy(&buf[..len as usize]),
            pid,
        });
        1
    }
}

/// Every attempt is appended to %LOCALAPPDATA%\deckhand\reveal.log so a
/// "clicking did nothing" report can be diagnosed after the fact: what
/// was searched for, which host it was classified as, which strategy
/// ran, and what it produced.
#[cfg(windows)]
fn log_attempt(
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
    host: Option<Host>,
    strategy: &str,
    windows_seen: usize,
    ambiguous: bool,
    result: &str,
) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let path = std::path::Path::new(&base).join("deckhand").join("reveal.log");
    let line = format!(
        "label={label:?} dir={dir:?} pid={pid:?} host={host:?} strategy={strategy} windows={windows_seen} ambiguous={ambiguous} result={result:?}\n"
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// Remove every window owned by this very process before scoring
/// (PR review P2). Without this, Deckhand's own board can tie a real
/// host on label or dir text and, as whichever candidate EnumWindows
/// happened to visit first, win the tie outright. Filtering by pid
/// rather than by title means a project actually named "Deckhand" is
/// still found correctly; only this process's own window is ever
/// excluded.
pub fn exclude_own_process(windows: Vec<Candidate>, own_pid: u32) -> Vec<Candidate> {
    windows.into_iter().filter(|c| c.pid != own_pid).collect()
}

/// Every candidate that scores above zero against `label`, `dir`, and
/// `pid`, in the order given. Shared by `pick` and by the reveal paths
/// that need to know, for the log, whether a miss was "nothing scored"
/// or "the top score was tied."
fn score_all<'a>(
    windows: &'a [Candidate],
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
) -> Vec<(i32, &'a Candidate)> {
    let label_lc = label.to_lowercase();
    let dir_lc = dir.map(str::to_lowercase);

    let mut scored = Vec::new();
    for c in windows {
        let title_lc = c.title.to_lowercase();
        let mut score = 0;
        if let Some(p) = pid {
            if c.pid == p {
                score += 4;
            }
        }
        if !label_lc.is_empty() && title_lc.contains(&label_lc) {
            score += 2;
        }
        if let Some(d) = &dir_lc {
            if !d.is_empty() && title_lc.contains(d.as_str()) {
                score += 2;
            }
        }
        if score > 0 {
            scored.push((score, c));
        }
    }
    scored
}

/// The unique best-scoring candidate, and whether the miss (if any) was
/// caused by a tie at the top score rather than by nothing scoring at
/// all. A pid match is worth more than either text match because it is
/// exact rather than a guess. A tie at the top score is ambiguous, not
/// a coin flip: two windows are equally plausible and guessing between
/// them (by taking whichever EnumWindows happened to visit first) is
/// exactly the kind of silent wrong answer Reveal exists to avoid, so
/// it is treated the same as no match at all.
fn pick_scored<'a>(
    windows: &'a [Candidate],
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
) -> (Option<(i32, &'a Candidate)>, bool) {
    let scored = score_all(windows, label, dir, pid);
    let Some(max) = scored.iter().map(|(s, _)| *s).max() else {
        return (None, false);
    };
    let mut at_max = scored.into_iter().filter(|(s, _)| *s == max);
    let first = at_max.next().expect("max came from a non-empty scored list");
    if at_max.next().is_some() {
        (None, true)
    } else {
        (Some(first), false)
    }
}

/// Score every candidate against a session's label, cwd directory name,
/// and pid, and return the best, or `None` on no match or an ambiguous
/// tie. See `pick_scored` for the full rule; this is the plain
/// `Option`-returning form the tests and every scored-match call site
/// use when they do not also need the ambiguous flag.
pub fn pick<'a>(
    windows: &'a [Candidate],
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
) -> Option<(i32, &'a Candidate)> {
    pick_scored(windows, label, dir, pid).0
}

/// The shared miss sentence every scored-match call site returns on no
/// match, kept in one place so `reveal_console`'s fallback,
/// `reveal_vscode`'s miss, and `reveal`'s own unknown-pid path can never
/// drift apart.
#[cfg(windows)]
fn no_match_sentence(label: &str) -> String {
    format!("No window matched \"{label}\". Reveal is a title and pid heuristic; the session may have no window on this machine.")
}

/// Restore if minimized, then attempt to raise `hwnd` with the ALT-tap
/// workaround (a background process is normally refused
/// SetForegroundWindow; the tap satisfies the "recent input" rule, and
/// is standard and ugly in equal measure). Every host path funnels its
/// winning window through this one function so the raise mechanics, and
/// the "refused the raise" sentence, live in exactly one place.
#[cfg(windows)]
fn raise(hwnd: HWND, title: &str) -> String {
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        keybd_event(VK_MENU as u8, 0, 0, 0);
        let ok = SetForegroundWindow(hwnd);
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
        if ok != 0 {
            format!("Raised \"{title}\".")
        } else {
            format!("Found \"{title}\" but Windows refused the raise.")
        }
    }
}

/// Pick the best scored match among `candidates` and turn it into a
/// raise or an honest miss sentence. Returns the sentence plus whether
/// the miss (if any) was an ambiguous tie, for the log line only.
#[cfg(windows)]
fn scored_match(
    label: &str,
    candidates: &[Candidate],
    dir: Option<&str>,
    pid: Option<u32>,
) -> (String, bool) {
    let (best, ambiguous) = pick_scored(candidates, label, dir, pid);
    let sentence = match best {
        Some((_, c)) => raise(c.hwnd as HWND, &c.title),
        None => no_match_sentence(label),
    };
    (sentence, ambiguous)
}

/// AttachConsole is process-global: only one console can be attached to
/// this process at a time, so every call is serialized through this
/// lock rather than let two Reveals race each other's attach/detach.
#[cfg(windows)]
static CONSOLE_LOCK: Mutex<()> = Mutex::new(());

/// A plain console host (cmd, PowerShell) is a 1:1 mapping: the session's
/// pid owns exactly one console window, found by briefly attaching to
/// it. No title or score involved, so there is nothing to get wrong
/// here except the attach itself failing.
#[cfg(windows)]
fn console_hwnd_for(pid: u32) -> Option<HWND> {
    let _guard = CONSOLE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return None;
        }
        let hwnd = GetConsoleWindow();
        FreeConsole();
        if hwnd.is_null() || IsWindowVisible(hwnd) == 0 {
            None
        } else {
            Some(hwnd)
        }
    }
}

#[cfg(windows)]
fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

#[cfg(windows)]
fn reveal_console(pid: u32, label: &str, dir: Option<&str>, windows: &[Candidate]) -> (String, bool) {
    if let Some(hwnd) = console_hwnd_for(pid) {
        let title = window_title(hwnd);
        let title = if title.is_empty() { label } else { &title };
        return (raise(hwnd, title), false);
    }
    // AttachConsole failed (the session may already have exited, or the
    // handle is otherwise unavailable): fall back to the same scored
    // title match every other host uses as a last resort.
    scored_match(label, windows, dir, Some(pid))
}

#[cfg(windows)]
fn reveal_windows_terminal(label: &str, windows: &[Candidate], procs: &HashMap<u32, (u32, String)>) -> String {
    let terminals: Vec<&Candidate> = windows
        .iter()
        .filter(|c| {
            procs
                .get(&c.pid)
                .map(|(_, exe)| exe.eq_ignore_ascii_case("windowsterminal.exe"))
                .unwrap_or(false)
        })
        .collect();
    match terminals.as_slice() {
        [] => no_match_sentence(label),
        [only] => raise(only.hwnd as HWND, &only.title),
        _ => format!("Found \"{label}\" in Windows Terminal, but more than one Terminal window is open."),
    }
}

/// One `~/.claude/ide/*.lock` file's fields Reveal cares about. Every
/// other field (transport, ideName beyond confirming VS Code, and in
/// particular the bearer `authToken`) is parsed and discarded before
/// this struct is built, and must never reach a log line.
#[derive(Debug, Clone)]
pub struct IdeLock {
    pub pid: u32,
    pub workspace_folders: Vec<String>,
}

/// A path in a form fit for comparison: lowercased (Windows paths are
/// case-insensitive), backslashes turned to forward slashes, and a
/// trailing separator trimmed, so `C:\Foo\` and `c:/foo` agree.
fn normalize_path(p: &str) -> String {
    let s = p.trim().replace('\\', "/").to_lowercase();
    s.trim_end_matches('/').to_string()
}

/// True when `ancestor` is `path` itself or a directory that contains
/// it, both already normalised. Segment-bounded so `/dev/a` can never
/// match inside `/dev/ab`.
fn is_ancestor_of(ancestor: &str, path: &str) -> bool {
    ancestor == path || (path.starts_with(ancestor) && path.as_bytes().get(ancestor.len()) == Some(&b'/'))
}

/// The workspace folder, across every lock file VS Code's Claude Code
/// extension currently has open, that best fits a session's `cwd`: an
/// exact match, or its longest ancestor directory. Two folders tied at
/// the same length, whether they come from the same lock file or two
/// different ones, are ambiguous: two windows would be equally
/// plausible, and Reveal never guesses between equally plausible
/// windows (the same rule `pick_scored` applies to titles), so a tie
/// returns `None` exactly like no match at all. Pure data in, data out
/// so it is tested without touching the filesystem; `read_ide_locks` is
/// the cfg(windows) half that actually reads the lock files.
pub fn match_workspace<'a>(locks: &'a [IdeLock], cwd: &str) -> Option<(u32, &'a str)> {
    let cwd_n = normalize_path(cwd);
    if cwd_n.is_empty() {
        return None;
    }
    let mut best_len: Option<usize> = None;
    let mut winners: Vec<(u32, &str)> = Vec::new();
    for lock in locks {
        for folder in &lock.workspace_folders {
            let folder_n = normalize_path(folder);
            if folder_n.is_empty() || !is_ancestor_of(&folder_n, &cwd_n) {
                continue;
            }
            let len = folder_n.len();
            let is_new_best = match best_len {
                None => true,
                Some(b) => len > b,
            };
            if is_new_best {
                best_len = Some(len);
                winners.clear();
                winners.push((lock.pid, folder.as_str()));
            } else if best_len == Some(len) {
                winners.push((lock.pid, folder.as_str()));
            }
        }
    }
    match winners.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// Read every `%USERPROFILE%\.claude\ide\*.lock` file into the shape
/// `match_workspace` needs. Best-effort throughout: a missing
/// directory, an unreadable file, or a file that fails to parse into
/// the expected shape is simply skipped rather than failing the whole
/// lookup, since one stray or half-written lock file must never break
/// Reveal for every other session.
#[cfg(windows)]
fn read_ide_locks() -> Vec<IdeLock> {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return Vec::new();
    };
    let dir = std::path::Path::new(&home).join(".claude").join("ide");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut locks = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lock") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(pid) = value.get("pid").and_then(|v| v.as_u64()) else {
            continue;
        };
        let workspace_folders = value
            .get("workspaceFolders")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        locks.push(IdeLock { pid: pid as u32, workspace_folders });
    }
    locks
}

/// The full image path of a running process, via
/// QueryFullProcessImageNameW. `None` on any failure (the process has
/// already exited, or this process lacks the rights to query it).
#[cfg(windows)]
fn process_image_path(pid: u32) -> Option<std::path::PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 || len == 0 {
            return None;
        }
        Some(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
    }
}

/// `<the VS Code install directory>\bin\code.cmd`, resolved from the
/// image path of the Code.exe process at `ancestor_pid`
/// (`host::vscode_ancestor_pid`). `None` when the image path cannot be
/// read or the CLI shim simply is not there (a portable or unusual
/// install); the caller treats either the same as "no CLI available"
/// and falls back to a title-only raise.
#[cfg(windows)]
fn code_cmd_path(ancestor_pid: u32) -> Option<std::path::PathBuf> {
    let exe = process_image_path(ancestor_pid)?;
    let cmd = exe.parent()?.join("bin").join("code.cmd");
    cmd.is_file().then_some(cmd)
}

/// Run `code.cmd "<folder>"` hidden and wait up to five seconds for it
/// to exit. VS Code's own CLI already focuses an already-open folder's
/// window when invoked again, so this call alone typically does most of
/// Reveal's work here; `reveal_vscode` still layers its own title raise
/// on top where a title match makes that possible. A timeout does not
/// necessarily mean failure (the CLI can be slow handing back from a
/// detached shell), so it is logged and otherwise ignored rather than
/// treated as an error.
#[cfg(windows)]
fn run_code_cli(code_cmd: &std::path::Path, folder: &str) -> Option<i32> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut child = std::process::Command::new(code_cmd)
        .arg(folder)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            _ => return None,
        }
    }
}

/// A second, VS Code-specific line appended right after `log_attempt`'s
/// line for the same click: the extra decision points `log_attempt`'s
/// fixed columns have no room for (which lock file matched, whether the
/// CLI ran and how it exited, whether the session-tab link was used).
/// Same file, same best-effort append; never includes a lock file's
/// authToken or any other field beyond what was already derived.
#[cfg(windows)]
fn log_vscode_detail(session_id: &str, lock_match: &str, cli: &str, link: &str) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let path = std::path::Path::new(&base).join("deckhand").join("reveal.log");
    let line =
        format!("  vscode: session={session_id:?} lock_match={lock_match:?} cli={cli:?} link={link:?}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// The VS Code path. `~/.claude/ide/*.lock` names every workspace
/// folder VS Code's Claude Code extension currently has open; when one
/// fits this session's `cwd` (`match_workspace`), this runs VS Code's
/// own CLI against that exact folder (which focuses an already-open
/// window by itself) and then restricts the title raise to windows
/// whose title names that folder, rather than the session's possibly
/// unrelated label. No match falls back to the pre-lock-file behaviour:
/// the scored title match restricted to Code.exe-owned windows.
///
/// The session-tab link
/// (`vscode://anthropic.claude-code/open?session=<id>`) that Step A was
/// asked to evaluate is deliberately NOT wired here, on the evidence
/// read out of extension.js: `createPanel` reveals an existing panel
/// instead of duplicating it only when the *receiving* window's
/// extension host already tracks that session id in its in-memory
/// `sessionPanels` map; any id it does not track falls through to
/// creating a brand-new panel, which every other line of that function
/// treats as a fresh `claude --resume` process. Two gaps this bundle
/// cannot close: (1) a session running in VS Code's own integrated
/// terminal is never tracked there (its immediate parent is a shell,
/// not the extension host, which `host::parent_is_vscode_exe` records
/// below for the log, though nothing here acts on it yet); (2) which
/// window `code.cmd --open-url` reaches when more than one VS Code
/// window has the extension active is decided by VS Code's own core URL
/// routing, which lives outside extension.js and was not observed.
/// Firing the link into either gap spawns a second live session instead
/// of revealing the first one, so until a real multi-window VS Code
/// confirms otherwise, the window raise below is Reveal's only VS Code
/// affordance.
#[cfg(windows)]
fn reveal_vscode(label: &str, dir: Option<&str>, cwd: Option<&str>, pid: u32, session_id: &str) -> (String, bool) {
    let mut windows: Vec<Candidate> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut windows as *mut _ as LPARAM);
    }
    let windows = exclude_own_process(windows, std::process::id());
    let procs = host::snapshot_processes();
    let vscode_windows: Vec<Candidate> = windows
        .into_iter()
        .filter(|c| procs.get(&c.pid).map(|(_, exe)| host::is_vscode_exe(exe)).unwrap_or(false))
        .collect();

    let extension_hosted = host::parent_is_vscode_exe(pid, &procs);
    let link_note = format!(
        "not wired regardless of context (duplication risk not ruled out; this session {} extension-hosted)",
        if extension_hosted { "is" } else { "is not" }
    );

    let locks = read_ide_locks();
    let matched = cwd.and_then(|c| match_workspace(&locks, c));

    let Some((_, folder)) = matched else {
        // No lock file names a folder that fits this cwd (extension not
        // running, folder not open, or an ambiguous tie between two
        // equally-fitting folders): the pid-blind scored title match is
        // still the best guess.
        let (result, ambiguous) = scored_match(label, &vscode_windows, dir, None);
        log_vscode_detail(session_id, "none", "not attempted (no folder match)", &link_note);
        return (result, ambiguous);
    };

    let basename = crate::state::dir_name(folder);
    let ancestor_pid = host::vscode_ancestor_pid(pid, &procs);
    let code_cmd = ancestor_pid.and_then(code_cmd_path);
    // Only a clean exit counts as the CLI having focused the folder's
    // window; a timeout or a non-zero exit is no evidence of a raise.
    let cli_exit = code_cmd.as_ref().map(|cmd| run_code_cli(cmd, folder));
    let cli_ran = matches!(cli_exit, Some(Some(0)));
    let cli_note = match cli_exit {
        Some(Some(code)) => format!("ran, exit={code}"),
        Some(None) => "ran, timed out or exit unknown".to_string(),
        None => "skipped (code.cmd not found)".to_string(),
    };

    let title_hits: Vec<&Candidate> = vscode_windows
        .iter()
        .filter(|c| c.title.to_lowercase().contains(&basename.to_lowercase()))
        .collect();

    let (result, ambiguous) = match title_hits.as_slice() {
        [one] => (raise(one.hwnd as HWND, &one.title), false),
        [] if cli_ran => (format!("Raised \"{basename}\" in VS Code."), false),
        [] => (no_match_sentence(label), false),
        _many if cli_ran => (format!("Raised \"{basename}\" in VS Code."), false),
        _many => (
            format!("Found \"{basename}\" in VS Code, but more than one matching window is open."),
            true,
        ),
    };

    log_vscode_detail(session_id, &format!("{folder:?} (pid {pid})"), &cli_note, &link_note);
    (result, ambiguous)
}

/// Try to raise the window for a session. `label` is the tile label,
/// `dir` the cwd directory name, `cwd` the session's raw cwd (used by
/// the VS Code path's lock-file lookup), `pid` the enumeration's pid
/// where one is known, `session_id` the session's own id (used by the
/// VS Code path's log line only today; see `reveal_vscode`). Returns a
/// sentence for the detail panel either way.
#[cfg(windows)]
pub fn reveal(label: &str, dir: Option<&str>, cwd: Option<&str>, pid: Option<u32>, session_id: &str) -> String {
    let mut windows: Vec<Candidate> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut windows as *mut _ as LPARAM);
    }
    let windows = exclude_own_process(windows, std::process::id());
    let windows_seen = windows.len();

    let Some(pid) = pid else {
        // No pid at all: the original scored title match, unchanged.
        let (result, ambiguous) = scored_match(label, &windows, dir, None);
        log_attempt(label, dir, None, None, "unknown-pid", windows_seen, ambiguous, &result);
        return result;
    };

    let host = host::classify(pid, &host::snapshot_processes());
    let (strategy, result, ambiguous) = match host {
        Host::Console => {
            let (result, ambiguous) = reveal_console(pid, label, dir, &windows);
            ("console", result, ambiguous)
        }
        Host::WindowsTerminal => {
            let procs = host::snapshot_processes();
            let result = reveal_windows_terminal(label, &windows, &procs);
            ("windows-terminal", result, false)
        }
        Host::VsCode => {
            let (result, ambiguous) = reveal_vscode(label, dir, cwd, pid, session_id);
            ("vscode", result, ambiguous)
        }
    };
    log_attempt(label, dir, Some(pid), Some(host), strategy, windows_seen, ambiguous, &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(hwnd: isize, title: &str, pid: u32) -> Candidate {
        Candidate { hwnd, title: title.to_string(), pid }
    }

    #[test]
    fn pid_match_beats_a_title_only_match() {
        let windows = vec![c(1, "deckhand - Visual Studio Code", 111), c(2, "unrelated window", 222)];
        let (score, best) = pick(&windows, "deckhand", None, Some(222)).unwrap();
        assert_eq!(best.hwnd, 2);
        assert_eq!(score, 4);
    }

    #[test]
    fn label_plus_dir_beats_label_alone() {
        let windows = vec![c(1, "deckhand", 1), c(2, "deckhand - undertow", 2)];
        let (score, best) = pick(&windows, "deckhand", Some("undertow"), None).unwrap();
        assert_eq!(best.hwnd, 2);
        assert_eq!(score, 4);
    }

    #[test]
    fn the_title_match_is_case_insensitive() {
        let windows = vec![c(1, "DECKHAND Session", 1)];
        let (score, best) = pick(&windows, "deckhand", None, None).unwrap();
        assert_eq!(best.hwnd, 1);
        assert_eq!(score, 2);
    }

    #[test]
    fn empty_label_none_dir_none_pid_gives_none() {
        let windows = vec![c(1, "anything at all", 1)];
        assert!(pick(&windows, "", None, None).is_none());
    }

    #[test]
    fn a_tie_at_the_top_score_is_ambiguous_and_matches_nothing() {
        // Two equally-titled windows must never be resolved by pick
        // order: guessing between them is exactly the silent wrong
        // answer Reveal exists to avoid.
        let windows = vec![c(1, "deckhand", 1), c(2, "deckhand", 2)];
        assert!(pick(&windows, "deckhand", None, None).is_none(), "a tie must return no match, not the first candidate");
        let (best, ambiguous) = pick_scored(&windows, "deckhand", None, None);
        assert!(best.is_none());
        assert!(ambiguous, "the miss must be logged as an ambiguous tie, not a plain no-match");
    }

    #[test]
    fn a_pid_owning_no_listed_window_falls_through_to_the_title_candidate() {
        let windows = vec![c(1, "deckhand", 1)];
        let (score, best) = pick(&windows, "deckhand", None, Some(999)).unwrap();
        assert_eq!(best.hwnd, 1);
        assert_eq!(score, 2, "the missed pid contributes nothing, only the title match");
    }

    #[test]
    fn a_candidate_with_score_zero_is_never_returned() {
        let windows = vec![c(1, "unrelated", 1)];
        assert!(pick(&windows, "deckhand", Some("undertow"), Some(2)).is_none());
    }

    #[test]
    fn a_non_ambiguous_miss_does_not_report_ambiguous() {
        let windows = vec![c(1, "unrelated", 1)];
        let (best, ambiguous) = pick_scored(&windows, "deckhand", Some("undertow"), Some(2));
        assert!(best.is_none());
        assert!(!ambiguous, "nothing scored at all is a plain miss, not a tie");
    }

    // ---- PR review P2: Deckhand's own window must never win Reveal -----

    #[test]
    fn excluding_the_own_process_lets_a_tied_valid_host_win() {
        const OWN_PID: u32 = 1000;
        // The board itself is enumerated first and ties the real host on
        // the label text alone; without filtering, that tie is
        // ambiguous and must not resolve to the board just because
        // EnumWindows visited it first.
        let windows = vec![c(1, "deckhand", OWN_PID), c(2, "deckhand - undertow", 2000)];
        assert!(
            pick(&windows, "deckhand", None, None).is_none(),
            "sanity: without filtering, the tie is ambiguous"
        );

        let filtered = exclude_own_process(windows, OWN_PID);
        let (_, best) = pick(&filtered, "deckhand", None, None).unwrap();
        assert_eq!(best.hwnd, 2, "the board must never win a Reveal, and excluding it resolves the tie");
    }

    #[test]
    fn exclude_own_process_leaves_every_other_window_untouched() {
        let windows = vec![c(1, "a", 1), c(2, "b", 2), c(3, "c", 3)];
        let filtered = exclude_own_process(windows, 999);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn exclude_own_process_can_remove_more_than_one_window_of_its_own() {
        let windows = vec![c(1, "deckhand", 42), c(2, "deckhand settings", 42), c(3, "host", 7)];
        let filtered = exclude_own_process(windows, 42);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].hwnd, 3);
    }

    // ---- match_workspace (VS Code lock-file lookup) ---------------------

    fn lock(pid: u32, folders: &[&str]) -> IdeLock {
        IdeLock { pid, workspace_folders: folders.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn an_exact_match_wins() {
        let locks = vec![lock(1, &[r"C:\Users\o\dev\deckhand"])];
        let (pid, folder) = match_workspace(&locks, r"C:\Users\o\dev\deckhand").unwrap();
        assert_eq!(pid, 1);
        assert_eq!(folder, r"C:\Users\o\dev\deckhand");
    }

    #[test]
    fn a_cwd_deeper_than_the_workspace_root_matches_its_ancestor() {
        let locks = vec![lock(1, &[r"C:\Users\o\dev\deckhand"])];
        let (pid, folder) = match_workspace(&locks, r"C:\Users\o\dev\deckhand\app\src-tauri").unwrap();
        assert_eq!(pid, 1);
        assert_eq!(folder, r"C:\Users\o\dev\deckhand");
    }

    #[test]
    fn case_and_separator_differences_still_match() {
        let locks = vec![lock(1, &["c:/users/o/dev/deckhand/"])];
        let (pid, _) = match_workspace(&locks, r"C:\Users\O\dev\DECKHAND").unwrap();
        assert_eq!(pid, 1);
    }

    #[test]
    fn the_longest_ancestor_wins_over_a_shorter_one() {
        // A monorepo root and a nested project both open, in two
        // different windows; the session's cwd is inside the nested
        // one, which must win even though the root also qualifies.
        let locks = vec![lock(1, &[r"C:\dev\mono"]), lock(2, &[r"C:\dev\mono\pkg"])];
        let (pid, folder) = match_workspace(&locks, r"C:\dev\mono\pkg\src").unwrap();
        assert_eq!(pid, 2);
        assert_eq!(folder, r"C:\dev\mono\pkg");
    }

    #[test]
    fn a_sibling_directory_with_a_shared_prefix_never_matches() {
        // "/dev/deckhand" must not be treated as an ancestor of
        // "/dev/deckhand-other": a naive prefix compare would get this
        // wrong.
        let locks = vec![lock(1, &[r"C:\dev\deckhand"])];
        assert!(match_workspace(&locks, r"C:\dev\deckhand-other").is_none());
    }

    #[test]
    fn no_open_folder_contains_cwd_is_no_match() {
        let locks = vec![lock(1, &[r"C:\dev\other-project"])];
        assert!(match_workspace(&locks, r"C:\dev\deckhand").is_none());
    }

    #[test]
    fn an_empty_cwd_is_no_match() {
        let locks = vec![lock(1, &[r"C:\dev\deckhand"])];
        assert!(match_workspace(&locks, "").is_none());
    }

    #[test]
    fn two_locks_listing_the_same_folder_is_ambiguous() {
        // Two different VS Code windows (say, a stale lock left behind
        // by a crashed extension host alongside its live replacement)
        // both claim the exact same folder: neither is more plausible
        // than the other, so this must miss rather than guess.
        let locks = vec![lock(1, &[r"C:\dev\deckhand"]), lock(2, &[r"C:\dev\deckhand"])];
        assert!(match_workspace(&locks, r"C:\dev\deckhand").is_none());
    }

    #[test]
    fn a_tie_still_applies_when_the_match_is_by_ancestor_not_exact() {
        // The same duplicate-lock ambiguity as the exact-match case
        // above, but reached through the longest-ancestor rule instead,
        // pinning that the tie-break is on length, not on being an
        // exact match.
        let locks = vec![lock(1, &[r"C:\dev\deckhand"]), lock(2, &[r"C:\dev\deckhand"])];
        assert!(match_workspace(&locks, r"C:\dev\deckhand\app").is_none());
    }
}
