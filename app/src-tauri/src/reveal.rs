// Reveal: raise a session's host window (docs/CONTROL_MAPPING.md).
//
// Matching strategy per ADR-023: prefer the session's pid where a
// top-level window is actually owned by it, fall back to matching the
// workspace or directory name in the window title, which is the only
// route on a vscode-extension host where every window shares one
// process, and a decent heuristic for terminals. Confidence is
// synthetic; a miss returns an honest sentence for the detail panel
// instead of pretending.
//
// Scoring (`pick`) is plain data in, data out, so it is tested without a
// window manager. Everything that actually touches Win32 is cfg-gated
// per item instead of at module scope, so `Candidate` and `pick` build
// and test on any host.

#[cfg(windows)]
use windows_sys::Win32::Foundation::{HWND, LPARAM};
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, SW_RESTORE,
};

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
/// was searched for, how many windows were seen, and what won.
#[cfg(windows)]
fn log_attempt(
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
    windows: &[Candidate],
    best: Option<(i32, String)>,
) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let path = std::path::Path::new(&base).join("deckhand").join("reveal.log");
    let line = format!(
        "label={label:?} dir={dir:?} pid={pid:?} windows={} best={best:?}\n",
        windows.len()
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// Score every candidate against a session's label, cwd directory name,
/// and pid, and return the best. A pid match is worth more than either
/// text match because it is exact rather than a guess; a candidate with
/// no match at all (score 0) is never returned, and ties keep whichever
/// candidate was seen first.
pub fn pick<'a>(
    windows: &'a [Candidate],
    label: &str,
    dir: Option<&str>,
    pid: Option<u32>,
) -> Option<(i32, &'a Candidate)> {
    let label_lc = label.to_lowercase();
    let dir_lc = dir.map(str::to_lowercase);

    let mut best: Option<(i32, &Candidate)> = None;
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
            let better = match best {
                Some((s, _)) => score > s,
                None => true,
            };
            if better {
                best = Some((score, c));
            }
        }
    }
    best
}

/// Try to raise the window for a session. `label` is the tile label,
/// `dir` the cwd directory name, `pid` the enumeration's pid where one
/// is known. Returns a sentence for the detail panel either way.
#[cfg(windows)]
pub fn reveal(label: &str, dir: Option<&str>, pid: Option<u32>) -> String {
    let mut windows: Vec<Candidate> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut windows as *mut _ as LPARAM);
    }

    let best = pick(&windows, label, dir, pid);

    log_attempt(label, dir, pid, &windows, best.as_ref().map(|(s, c)| (*s, c.title.clone())));

    let Some((_, target)) = best else {
        return format!("No window matched \"{label}\". Reveal is a title and pid heuristic; the session may have no window on this machine.");
    };

    unsafe {
        if IsIconic(target.hwnd as HWND) != 0 {
            ShowWindow(target.hwnd as HWND, SW_RESTORE);
        }
        // A background process is normally refused SetForegroundWindow.
        // The ALT tap satisfies the "recent input" rule; standard and
        // ugly in equal measure.
        keybd_event(VK_MENU as u8, 0, 0, 0);
        let ok = SetForegroundWindow(target.hwnd as HWND);
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
        if ok != 0 {
            format!("Raised \"{}\".", target.title)
        } else {
            format!("Found \"{}\" but Windows refused the raise.", target.title)
        }
    }
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
    fn ties_keep_the_first_candidate() {
        let windows = vec![c(1, "deckhand", 1), c(2, "deckhand", 2)];
        let (_, best) = pick(&windows, "deckhand", None, None).unwrap();
        assert_eq!(best.hwnd, 1, "the first candidate wins a tie");
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
}
