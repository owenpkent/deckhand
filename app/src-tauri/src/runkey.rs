// "Start with Windows," the settings panel's HKCU Run-key toggle
// (docs/DECISIONS.md#adr-033). The registry value named `Deckhand`
// under `Software\Microsoft\Windows\CurrentVersion\Run` is the single
// source of truth: nothing about this setting is mirrored into
// settings.json, so a copy of Deckhand run once by hand and never
// launched again cannot leave a stale "on" behind anywhere else.
// Comparing what is there against this install's own exe path is a
// pure function, unit-tested without a real registry; the registry
// calls themselves are the impure edge, gated `#[cfg(windows)]` like
// every other Win32-specific module in this daemon.
//
// docs/SECURITY_MODEL.md's "touch other tools' config like a guest"
// rule applies here exactly as it does to settings.json: Deckhand reads
// and writes only its own named value, never anything else under Run,
// and only on an explicit click.

pub const RUN_KEY_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
pub const RUN_VALUE_NAME: &str = "Deckhand";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum StartWithWindowsState {
    Off,
    OnThisExe,
    /// The Run value names some other exe path, e.g. a second install
    /// or a moved copy of this one. The panel shows this as "On (other
    /// copy)"; clicking it points the value at this exe instead of
    /// turning it off, since a person choosing "start with Windows" for
    /// Deckhand almost never means "for whichever copy is already
    /// there."
    OnOtherExe { path: String },
}

/// The exact string Deckhand writes into the Run value: the exe path,
/// double-quoted, so a path containing spaces still runs as one
/// argument the same way Explorer's own startup handling would launch
/// it.
pub fn run_value_for_exe(exe_path: &str) -> String {
    format!("\"{exe_path}\"")
}

fn strip_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

/// Compares the registry's current value (`None` when the value is
/// absent, the ordinary never-turned-on state) against this install's
/// own exe path. Case-insensitive, since Windows paths are, and
/// tolerant of an unquoted value: nothing Deckhand itself writes
/// unquoted, but nothing stops another tool, or a hand edit, having put
/// one there.
pub fn compare_run_value(current: Option<&str>, this_exe_path: &str) -> StartWithWindowsState {
    match current {
        None => StartWithWindowsState::Off,
        Some(raw) => {
            let existing = strip_quotes(raw);
            if existing.eq_ignore_ascii_case(this_exe_path) {
                StartWithWindowsState::OnThisExe
            } else {
                StartWithWindowsState::OnOtherExe { path: existing.to_string() }
            }
        }
    }
}

#[cfg(windows)]
mod win {
    use super::{RUN_KEY_SUBKEY, RUN_VALUE_NAME};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Reads the Run value's current string, or `None` when the key or
    /// the value does not exist, which is the ordinary "never turned
    /// on" state, not an error.
    pub fn read_run_value() -> Option<String> {
        unsafe {
            let subkey = wide(RUN_KEY_SUBKEY);
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != ERROR_SUCCESS {
                return None;
            }
            let name = wide(RUN_VALUE_NAME);
            // 32 KiB of UTF-16 is far beyond any real path; comfortably
            // covers even a very long one without a second, resized
            // call.
            let mut buf = [0u16; 16_384];
            let mut len_bytes = (buf.len() * 2) as u32;
            let mut kind = 0u32;
            let status = RegQueryValueExW(
                hkey,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                buf.as_mut_ptr() as *mut u8,
                &mut len_bytes,
            );
            RegCloseKey(hkey);
            if status != ERROR_SUCCESS || kind != REG_SZ {
                return None;
            }
            let chars = (len_bytes as usize / 2).min(buf.len());
            let raw = String::from_utf16_lossy(&buf[..chars]);
            Some(raw.trim_end_matches('\0').to_string())
        }
    }

    /// Writes the Run value. `Software\Microsoft\Windows\CurrentVersion\Run`
    /// always exists on a real Windows install, so this never needs to
    /// create the key itself, only open it.
    pub fn write_run_value(value: &str) -> std::io::Result<()> {
        unsafe {
            let subkey = wide(RUN_KEY_SUBKEY);
            let mut hkey: HKEY = std::ptr::null_mut();
            let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey);
            if status != ERROR_SUCCESS {
                return Err(std::io::Error::from_raw_os_error(status as i32));
            }
            let name = wide(RUN_VALUE_NAME);
            let data = wide(value);
            let data_len_bytes = (data.len() * 2) as u32;
            let status =
                RegSetValueExW(hkey, name.as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, data_len_bytes);
            RegCloseKey(hkey);
            if status != ERROR_SUCCESS {
                return Err(std::io::Error::from_raw_os_error(status as i32));
            }
            Ok(())
        }
    }

    /// Deletes the Run value. A value that is already absent is not an
    /// error from the caller's point of view: "make sure it is off"
    /// must not fail just because it already was off.
    pub fn delete_run_value() -> std::io::Result<()> {
        unsafe {
            let subkey = wide(RUN_KEY_SUBKEY);
            let mut hkey: HKEY = std::ptr::null_mut();
            let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey);
            if status != ERROR_SUCCESS {
                return Err(std::io::Error::from_raw_os_error(status as i32));
            }
            let name = wide(RUN_VALUE_NAME);
            let status = RegDeleteValueW(hkey, name.as_ptr());
            RegCloseKey(hkey);
            if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
                return Err(std::io::Error::from_raw_os_error(status as i32));
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
pub use win::{delete_run_value, read_run_value, write_run_value};

#[cfg(not(windows))]
pub fn read_run_value() -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn write_run_value(_value: &str) -> std::io::Result<()> {
    Ok(())
}
#[cfg(not(windows))]
pub fn delete_run_value() -> std::io::Result<()> {
    Ok(())
}

/// Reads the real registry value and compares it against `this_exe_path`
/// in one call, for `main.rs`'s convenience.
pub fn current_state(this_exe_path: &str) -> StartWithWindowsState {
    compare_run_value(read_run_value().as_deref(), this_exe_path)
}

/// Applies the panel's toggle: turn on for this exe when the value is
/// currently off or points at a different copy, or off when it is
/// already on for this exe. Returns the state after the write, read
/// back from the value just written or deleted rather than assumed, so
/// a write that silently failed still reports what is actually there.
pub fn toggle(this_exe_path: &str) -> StartWithWindowsState {
    match current_state(this_exe_path) {
        StartWithWindowsState::OnThisExe => {
            let _ = delete_run_value();
        }
        StartWithWindowsState::Off | StartWithWindowsState::OnOtherExe { .. } => {
            let _ = write_run_value(&run_value_for_exe(this_exe_path));
        }
    }
    current_state(this_exe_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const THIS_EXE: &str = r"C:\Users\owen\dev\deckhand\target\debug\deckhand.exe";

    #[test]
    fn run_value_for_exe_is_double_quoted() {
        assert_eq!(run_value_for_exe(THIS_EXE), format!("\"{THIS_EXE}\""));
    }

    #[test]
    fn no_value_compares_off() {
        assert_eq!(compare_run_value(None, THIS_EXE), StartWithWindowsState::Off);
    }

    #[test]
    fn a_quoted_value_matching_this_exe_compares_on_this_exe() {
        let stored = format!("\"{THIS_EXE}\"");
        assert_eq!(compare_run_value(Some(&stored), THIS_EXE), StartWithWindowsState::OnThisExe);
    }

    #[test]
    fn an_unquoted_value_matching_this_exe_still_compares_on_this_exe() {
        assert_eq!(compare_run_value(Some(THIS_EXE), THIS_EXE), StartWithWindowsState::OnThisExe);
    }

    #[test]
    fn comparison_is_case_insensitive() {
        let stored = format!("\"{}\"", THIS_EXE.to_uppercase());
        assert_eq!(compare_run_value(Some(&stored), THIS_EXE), StartWithWindowsState::OnThisExe);
    }

    #[test]
    fn a_value_naming_a_different_exe_compares_on_other_exe() {
        let other = r"D:\old-deckhand\deckhand.exe";
        let stored = format!("\"{other}\"");
        assert_eq!(
            compare_run_value(Some(&stored), THIS_EXE),
            StartWithWindowsState::OnOtherExe { path: other.to_string() }
        );
    }

    // Pins the wire shape app/ui/src/types.ts's StartWithWindowsState
    // mirrors: a change here that is not also made there is a bug on
    // the surface, not just in this test.
    #[test]
    fn serializes_as_the_wire_shape_the_surface_expects() {
        assert_eq!(serde_json::to_string(&StartWithWindowsState::Off).unwrap(), r#"{"kind":"off"}"#);
        assert_eq!(serde_json::to_string(&StartWithWindowsState::OnThisExe).unwrap(), r#"{"kind":"onThisExe"}"#);
        assert_eq!(
            serde_json::to_string(&StartWithWindowsState::OnOtherExe { path: "x".to_string() }).unwrap(),
            r#"{"kind":"onOtherExe","path":"x"}"#
        );
    }
}
