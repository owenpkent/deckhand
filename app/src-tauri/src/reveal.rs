// Reveal: raise a session's host window (docs/CONTROL_MAPPING.md).
//
// Matching strategy per ADR-023: prefer the session's pid where a
// top-level window is actually owned by it, fall back to matching the
// workspace or directory name in the window title, which is the only
// route on a vscode-extension host where every window shares one
// process, and a decent heuristic for terminals. Confidence is
// synthetic; a miss returns an honest sentence for the detail panel
// instead of pretending.

#![cfg(windows)]

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_KEYUP, VK_MENU,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, SW_RESTORE,
};

struct Candidate {
    hwnd: isize,
    title: String,
    pid: u32,
}

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

/// Try to raise the window for a session. `label` is the tile label,
/// `dir` the cwd directory name, `pid` the enumeration's pid where one
/// is known. Returns a sentence for the detail panel either way.
pub fn reveal(label: &str, dir: Option<&str>, pid: Option<u32>) -> String {
    let mut windows: Vec<Candidate> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut windows as *mut _ as LPARAM);
    }

    let label_lc = label.to_lowercase();
    let dir_lc = dir.map(str::to_lowercase);

    let mut best: Option<(i32, &Candidate)> = None;
    for c in &windows {
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
