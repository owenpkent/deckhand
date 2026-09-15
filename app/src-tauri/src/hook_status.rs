// Whether the twelve-event Deckhand hook group
// (scripts/install-hooks.ps1) is registered in the user-level
// ~/.claude/settings.json, for the settings panel's Hooks row
// (docs/DECISIONS.md#adr-033). Parsing is a pure function of the
// file's text plus the current shim path, so "Installed," "Outdated,"
// "Missing," and "Unreadable" are unit-tested without ever touching a
// real settings.json. The impure edge (finding and reading the real
// file) is the thin `read_status` wrapper at the bottom.

use serde_json::Value;

/// The twelve events install-hooks.ps1 registers
/// (docs/CLAUDE_CODE_ADAPTER.md#hook-installation). Kept here too,
/// rather than shared with the PowerShell script, because there is no
/// mechanism to share it through; the two lists are kept in sync by
/// hand and by the test below that pins the count.
pub const EVENTS: [&str; 12] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "SubagentStart",
    "SubagentStop",
    "Notification",
    "PermissionDenied",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    /// Every one of `EVENTS` carries exactly the group
    /// install-hooks.ps1 would write today: one hook, the current shim
    /// path, single-quoted, no timeout.
    Installed,
    /// At least one event carries a Deckhand group, but not every event
    /// does, or one that is present is stale: a raw unquoted path (the
    /// legacy form), a leftover `timeout`, or an event install-hooks.ps1
    /// covers that has no group here at all.
    Outdated,
    /// No event carries a Deckhand group. Also what a missing
    /// settings.json, or one with no `hooks` key at all, reads as: there
    /// is nothing to call outdated.
    Missing,
    /// The file exists but is not valid JSON, or could not be read at
    /// all (rather than simply not existing).
    Unreadable,
}

enum GroupMatch {
    Current,
    Outdated,
}

/// Case- and slash-insensitive, matching install-hooks.ps1's own
/// `ConvertTo-CanonicalShimPath`/comparison, so a path spelled with
/// backslashes compares equal to the forward-slash form of the same
/// file.
fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Inverse of install-hooks.ps1's `ConvertTo-ShellCommandToken`: unwraps
/// a single-quoted shell token, undoing `'\''` escaping, or returns the
/// string unchanged if it was never quoted, exactly like the script's
/// own `ConvertFrom-ShellCommandToken`, so a legacy raw path still
/// compares correctly against the shim path.
fn unwrap_shell_token(command: &str) -> String {
    if command.len() >= 2 && command.starts_with('\'') && command.ends_with('\'') {
        command[1..command.len() - 1].replace("'\\''", "'")
    } else {
        command.to_string()
    }
}

fn is_quoted_token(command: &str) -> bool {
    command.len() >= 2 && command.starts_with('\'') && command.ends_with('\'')
}

/// Whether `group` (one entry of `hooks.<Event>`, an array in the real
/// file) is a Deckhand group for `shim_path_norm`, and if so, whether it
/// matches today's install-hooks.ps1 output exactly. `None` means this
/// group is not Deckhand's at all (a different tool's hook, or a shape
/// install-hooks.ps1 never writes, such as more than one hook in the
/// group).
fn group_match(group: &Value, shim_path_norm: &str) -> Option<GroupMatch> {
    let hooks = group.get("hooks")?.as_array()?;
    if hooks.len() != 1 {
        return None;
    }
    let hook = &hooks[0];
    let command = hook.get("command")?.as_str()?;
    if normalize(&unwrap_shell_token(command)) != shim_path_norm {
        return None;
    }
    let has_timeout = hook.get("timeout").is_some();
    if is_quoted_token(command) && !has_timeout {
        Some(GroupMatch::Current)
    } else {
        Some(GroupMatch::Outdated)
    }
}

/// Pure: takes the settings file's raw text (never reads it itself) and
/// the current shim path, and returns the status the panel's Hooks row
/// shows.
pub fn parse_hook_status(settings_json: &str, shim_path: &str) -> HookStatus {
    let Ok(root) = serde_json::from_str::<Value>(settings_json) else {
        return HookStatus::Unreadable;
    };
    let Some(hooks_obj) = root.get("hooks").and_then(Value::as_object) else {
        return HookStatus::Missing;
    };
    let shim_norm = normalize(shim_path);
    let mut any_present = false;
    let mut all_current = true;
    for event in EVENTS {
        let matched = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .and_then(|groups| groups.iter().find_map(|group| group_match(group, &shim_norm)));
        match matched {
            Some(GroupMatch::Current) => any_present = true,
            Some(GroupMatch::Outdated) => {
                any_present = true;
                all_current = false;
            }
            None => all_current = false,
        }
    }
    if all_current {
        HookStatus::Installed
    } else if any_present {
        HookStatus::Outdated
    } else {
        HookStatus::Missing
    }
}

/// `shim_path` for the shim built alongside `exe_path` (both land in the
/// same `target/debug` directory, see `scripts/build-app.ps1`), the same
/// default install-hooks.ps1 computes when `-ShimPath` is not given.
pub fn default_shim_path(exe_path: &std::path::Path) -> Option<std::path::PathBuf> {
    Some(exe_path.parent()?.join("deckhand-shim.exe"))
}

/// The real, impure read: `~/.claude/settings.json`, same file
/// install-hooks.ps1 writes. A missing file reads as `Missing`, the
/// same as a present file with nothing Deckhand installed; any other
/// read failure (permissions, for example) reads as `Unreadable`,
/// distinct from a parse error only in that the JSON parser never even
/// ran.
pub fn read_status(shim_path: &str) -> HookStatus {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return HookStatus::Unreadable;
    };
    let path = std::path::PathBuf::from(home).join(".claude").join("settings.json");
    read_status_at(&path, shim_path)
}

pub fn read_status_at(path: &std::path::Path, shim_path: &str) -> HookStatus {
    match std::fs::read_to_string(path) {
        Ok(body) => parse_hook_status(&body, shim_path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HookStatus::Missing,
        Err(_) => HookStatus::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIM: &str = "c:/repo/target/debug/deckhand-shim.exe";

    fn current_group() -> Value {
        serde_json::json!({
            "hooks": [{"type": "command", "command": "'c:/repo/target/debug/deckhand-shim.exe'", "async": true}]
        })
    }

    fn matcher_group() -> Value {
        serde_json::json!({
            "matcher": "*",
            "hooks": [{"type": "command", "command": "'c:/repo/target/debug/deckhand-shim.exe'", "async": true}]
        })
    }

    fn installed_settings() -> Value {
        let mut hooks = serde_json::Map::new();
        for event in EVENTS {
            let group = if ["PreToolUse", "PostToolUse", "PostToolUseFailure"].contains(&event) {
                matcher_group()
            } else {
                current_group()
            };
            hooks.insert(event.to_string(), Value::Array(vec![group]));
        }
        serde_json::json!({ "hooks": hooks })
    }

    #[test]
    fn twelve_events_pinned() {
        assert_eq!(EVENTS.len(), 12);
    }

    #[test]
    fn fully_installed_reads_installed() {
        let json = installed_settings().to_string();
        assert_eq!(parse_hook_status(&json, SHIM), HookStatus::Installed);
    }

    #[test]
    fn a_raw_unquoted_path_reads_outdated() {
        let mut settings = installed_settings();
        settings["hooks"]["SessionStart"][0]["hooks"][0]["command"] =
            Value::String("c:/repo/target/debug/deckhand-shim.exe".to_string());
        assert_eq!(parse_hook_status(&settings.to_string(), SHIM), HookStatus::Outdated);
    }

    #[test]
    fn a_leftover_timeout_reads_outdated() {
        let mut settings = installed_settings();
        settings["hooks"]["Stop"][0]["hooks"][0]["timeout"] = serde_json::json!(5);
        assert_eq!(parse_hook_status(&settings.to_string(), SHIM), HookStatus::Outdated);
    }

    #[test]
    fn a_missing_event_reads_outdated_when_others_are_current() {
        let mut settings = installed_settings();
        settings["hooks"].as_object_mut().unwrap().remove("SessionEnd");
        assert_eq!(parse_hook_status(&settings.to_string(), SHIM), HookStatus::Outdated);
    }

    #[test]
    fn no_hooks_key_reads_missing() {
        assert_eq!(parse_hook_status("{}", SHIM), HookStatus::Missing);
    }

    #[test]
    fn a_hooks_key_naming_none_of_the_twelve_events_reads_missing() {
        let settings = serde_json::json!({ "hooks": { "SomeOtherTool": [current_group()] } });
        assert_eq!(parse_hook_status(&settings.to_string(), SHIM), HookStatus::Missing);
    }

    #[test]
    fn a_different_tools_hook_on_the_same_event_reads_missing_not_outdated() {
        let other = serde_json::json!({
            "hooks": [{"type": "command", "command": "python other.py"}]
        });
        let settings = serde_json::json!({ "hooks": { "SessionStart": [other] } });
        assert_eq!(parse_hook_status(&settings.to_string(), SHIM), HookStatus::Missing);
    }

    #[test]
    fn invalid_json_reads_unreadable() {
        assert_eq!(parse_hook_status("not json", SHIM), HookStatus::Unreadable);
    }

    // Pins the wire shape app/ui/src/types.ts's HookStatus mirrors.
    #[test]
    fn serializes_as_lowercase_snake_case() {
        assert_eq!(serde_json::to_string(&HookStatus::Installed).unwrap(), "\"installed\"");
        assert_eq!(serde_json::to_string(&HookStatus::Outdated).unwrap(), "\"outdated\"");
        assert_eq!(serde_json::to_string(&HookStatus::Missing).unwrap(), "\"missing\"");
        assert_eq!(serde_json::to_string(&HookStatus::Unreadable).unwrap(), "\"unreadable\"");
    }

    #[test]
    fn shim_path_comparison_is_case_and_slash_insensitive() {
        let settings = installed_settings();
        let backslash_shim = r"C:\REPO\target\debug\DECKHAND-SHIM.exe";
        assert_eq!(parse_hook_status(&settings.to_string(), backslash_shim), HookStatus::Installed);
    }

    #[test]
    fn default_shim_path_is_a_sibling_of_the_exe() {
        let exe = std::path::Path::new("C:/repo/target/debug/deckhand.exe");
        let shim = default_shim_path(exe).unwrap();
        assert_eq!(shim, std::path::Path::new("C:/repo/target/debug/deckhand-shim.exe"));
    }

    #[test]
    fn read_status_at_a_missing_file_is_missing_not_unreadable() {
        let dir = std::env::temp_dir().join(format!("deckhand-hookstatus-test-{}", std::process::id()));
        let path = dir.join("does-not-exist.json");
        assert_eq!(read_status_at(&path, SHIM), HookStatus::Missing);
    }

    #[test]
    fn read_status_at_a_real_unreadable_file_is_unreadable() {
        let dir = std::env::temp_dir().join(format!("deckhand-hookstatus-test-{}-b", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read_status_at(&path, SHIM), HookStatus::Unreadable);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
