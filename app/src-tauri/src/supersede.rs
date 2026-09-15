// Supersession (docs/DECISIONS.md, ADR-038): the VS Code extension was
// seen keeping an earlier session's `claude.exe` alive, for hours, after
// the next one started in the same window, so `claude agents --json`
// lists both and the board shows what looks like a duplicate row. Why
// the older process outlives its conversation is unconfirmed. The older
// session is not wrong to list on its own; it is only wrong to show
// once a newer one has plainly taken over the same window and folder.
//
// `superseded` decides that, and nothing else: it does not unbind, does
// not touch `heard` or a session's colour, and does not remember
// anything between calls. registry.rs calls it fresh every time it
// needs the answer (building a snapshot, sizing the window), off
// whatever the registry currently holds, so a session that stops
// qualifying -- the older one goes `Thinking`, `NeedsInput`, or
// `Error`, or the newer one ends -- simply stops appearing here on the
// very next call. No extra bookkeeping, no separate "un-supersede"
// path to get wrong.

use std::collections::HashSet;

use crate::host::Host;
use crate::state::SessionState;

/// The handful of facts `superseded` needs about one bound session,
/// borrowed out of a `Session` (registry.rs builds these) rather than
/// depending on the full struct: keeping this narrow is what makes the
/// decision itself testable without constructing a real `Session`, a
/// `Registry`, or a process tree.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub id: &'a str,
    /// `None` until a scan has reported a pid for this session at least
    /// once; a session with no known pid can never take part either way
    /// (rule 1: "both have a known pid").
    pub pid: Option<u32>,
    /// This session's process's host classification (`host::resolve`),
    /// resolved once when its pid was learned. Only `Host::VsCode` on
    /// both sides can ever match; a plain console or a Windows Terminal
    /// tab is never superseded and never supersedes.
    pub host: Option<Host>,
    /// This session's process's own immediate OS parent pid, resolved
    /// in the same call as `host`. Two sessions sharing one VS Code
    /// extension host (one per window) share this value.
    pub parent_pid: Option<u32>,
    pub cwd: Option<&'a str>,
    pub state: SessionState,
    /// The scan's own `startedAt` (ms epoch), when the most recent scan
    /// to list this session reported one.
    pub started_at_ms: Option<i64>,
    /// When the daemon first heard of this session at all, by any
    /// channel: the fallback ordering when one side or the other has no
    /// `startedAt`.
    pub first_seen_ms: i64,
}

/// True when `o` is superseded by `n`: same VS Code window (matching
/// direct parent pid, both classified `Host::VsCode`), the same folder,
/// `n` started after `o`, and `o` sits in a state it is safe to hide
/// (never `Thinking`, `NeedsInput`, or `Error` -- a session doing
/// something or waiting on a human is never a stale duplicate). `n`
/// itself may be in any state except `Ended`: an ended session has
/// already handed nothing back, so it supersedes nothing.
fn is_superseded_by(o: &Candidate, n: &Candidate) -> bool {
    if o.id == n.id {
        return false;
    }
    if o.pid.is_none() || n.pid.is_none() {
        return false;
    }
    if o.host != Some(Host::VsCode) || n.host != Some(Host::VsCode) {
        return false;
    }
    let (Some(o_parent), Some(n_parent)) = (o.parent_pid, n.parent_pid) else {
        return false;
    };
    if o_parent != n_parent {
        return false;
    }
    let (Some(o_cwd), Some(n_cwd)) = (o.cwd, n.cwd) else {
        return false;
    };
    if crate::reveal::normalize_path(o_cwd) != crate::reveal::normalize_path(n_cwd) {
        return false;
    }
    if !matches!(o.state, SessionState::Idle | SessionState::Complete | SessionState::Unknown) {
        return false;
    }
    if n.state == SessionState::Ended {
        return false;
    }
    let (o_started, n_started) = match (o.started_at_ms, n.started_at_ms) {
        (Some(ot), Some(nt)) => (ot, nt),
        _ => (o.first_seen_ms, n.first_seen_ms),
    };
    n_started > o_started
}

/// Every id in `candidates` that some other, newer candidate makes safe
/// to hide right now. Pairwise (candidate counts are a handful of
/// sessions at most, never enough for the O(n^2) scan to matter) and
/// pure: the caller decides what "hide" means for its own list
/// (registry.rs keeps the binding, only leaves the id out of the
/// tiles/row-count it builds).
pub fn superseded(candidates: &[Candidate]) -> HashSet<String> {
    let mut hidden = HashSet::new();
    for o in candidates {
        if candidates.iter().any(|n| is_superseded_by(o, n)) {
            hidden.insert(o.id.to_string());
        }
    }
    hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c<'a>(
        id: &'a str,
        pid: u32,
        parent_pid: u32,
        cwd: &'a str,
        state: SessionState,
        started_at_ms: Option<i64>,
        first_seen_ms: i64,
    ) -> Candidate<'a> {
        Candidate {
            id,
            pid: Some(pid),
            host: Some(Host::VsCode),
            parent_pid: Some(parent_pid),
            cwd: Some(cwd),
            state,
            started_at_ms,
            first_seen_ms,
        }
    }

    fn ids(hidden: HashSet<String>) -> Vec<String> {
        let mut v: Vec<String> = hidden.into_iter().collect();
        v.sort();
        v
    }

    #[test]
    fn the_observed_four_session_case_hides_exactly_the_two_old_ones() {
        // The live 2026-09-15 report: two VS Code windows, each with an
        // idle session the extension kept alive after a newer one
        // started in the same window and folder.
        let deckhand_old = c(
            "deckhand-e4",
            37808,
            34208,
            r"c:\Users\owenp\dev\deckhand",
            SessionState::Idle,
            Some(1_789_492_719_945),
            1,
        );
        let deckhand_new = c(
            "deckhand-91",
            47412,
            34208,
            r"c:\Users\owenp\dev\deckhand",
            SessionState::Thinking,
            Some(1_789_508_532_480),
            2,
        );
        let atdev_old =
            c("atdev-marketing-12", 39500, 292, r"c:\Users\owenp\dev\ATDev-Marketing", SessionState::Idle, None, 3);
        let atdev_new =
            c("atdev-marketing-73", 44636, 292, r"c:\Users\owenp\dev\ATDev-Marketing", SessionState::Thinking, None, 4);
        let hidden = superseded(&[deckhand_old, deckhand_new, atdev_old, atdev_new]);
        assert_eq!(ids(hidden), vec!["atdev-marketing-12".to_string(), "deckhand-e4".to_string()]);
    }

    #[test]
    fn o_thinking_is_never_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Thinking, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty(), "a session doing something is never a stale duplicate");
    }

    #[test]
    fn o_needs_input_is_never_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::NeedsInput, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty(), "a session waiting on a human is never hidden out from under them");
    }

    #[test]
    fn o_error_is_never_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Error, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty());
    }

    #[test]
    fn o_complete_is_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Complete, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        assert_eq!(ids(superseded(&[o, n])), vec!["o".to_string()]);
    }

    #[test]
    fn o_unknown_is_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Unknown, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        assert_eq!(ids(superseded(&[o, n])), vec!["o".to_string()]);
    }

    #[test]
    fn a_different_parent_pid_is_never_superseded() {
        // Two different VS Code windows, each with its own extension
        // host: same folder open twice is a coincidence, not a
        // duplicate.
        let o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let n = c("n", 2, 999, "/dev/a", SessionState::Idle, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty());
    }

    #[test]
    fn a_different_cwd_is_never_superseded() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let n = c("n", 2, 100, "/dev/b", SessionState::Idle, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty());
    }

    #[test]
    fn a_cwd_differing_only_in_case_separator_and_trailing_slash_still_matches() {
        let o = c("o", 1, 100, r"C:\dev\Deckhand\", SessionState::Idle, Some(1), 1);
        let n = c("n", 2, 100, "c:/dev/deckhand", SessionState::Idle, Some(2), 2);
        assert_eq!(ids(superseded(&[o, n])), vec!["o".to_string()]);
    }

    #[test]
    fn a_non_vscode_host_is_never_superseded() {
        let mut o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let mut n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        o.host = Some(Host::Console);
        n.host = Some(Host::Console);
        assert!(superseded(&[o, n]).is_empty(), "a plain console or Windows Terminal tab is never a VS Code duplicate");
    }

    #[test]
    fn n_ended_means_o_still_shows() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Ended, Some(2), 2);
        assert!(superseded(&[o, n]).is_empty(), "an ended session hands nothing back; it supersedes nothing");
    }

    #[test]
    fn missing_started_at_falls_back_to_first_seen() {
        let older = c("older", 1, 100, "/dev/a", SessionState::Idle, None, 10);
        let newer = c("newer", 2, 100, "/dev/a", SessionState::Idle, None, 20);
        assert_eq!(ids(superseded(&[older, newer])), vec!["older".to_string()]);
    }

    #[test]
    fn started_at_is_only_used_when_both_sides_report_it() {
        // "older" has no startedAt at all; "newer" does, and it names a
        // moment earlier than "older"'s own first-seen time. If
        // startedAt leaked in from one side alone, this would flip the
        // ordering; instead both fall back to first-seen, so "older"
        // (first-seen 10) is still the one hidden, by "newer"
        // (first-seen 20).
        let older = c("older", 1, 100, "/dev/a", SessionState::Idle, None, 10);
        let mut newer = c("newer", 2, 100, "/dev/a", SessionState::Idle, None, 20);
        newer.started_at_ms = Some(1);
        assert_eq!(ids(superseded(&[older, newer])), vec!["older".to_string()]);
    }

    #[test]
    fn three_sessions_in_one_window_and_folder_only_the_newest_stays() {
        let s1 = c("s1", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let s2 = c("s2", 2, 100, "/dev/a", SessionState::Complete, Some(2), 2);
        let s3 = c("s3", 3, 100, "/dev/a", SessionState::Thinking, Some(3), 3);
        assert_eq!(ids(superseded(&[s1, s2, s3])), vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn o_returning_to_thinking_stops_being_superseded() {
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        let mut o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        assert_eq!(ids(superseded(&[o, n])), vec!["o".to_string()]);

        // A hook arrives (or the scan tiebreak flips it): the same pure
        // call, given the new state, shows it again with no other
        // change anywhere.
        o.state = SessionState::Thinking;
        assert!(superseded(&[o, n]).is_empty());
    }

    #[test]
    fn a_session_with_no_known_pid_is_never_superseded() {
        let mut o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        o.pid = None;
        assert!(superseded(&[o, n]).is_empty(), "o has no pid to match on");
    }

    #[test]
    fn a_session_with_no_known_pid_never_supersedes_another() {
        let o = c("o", 1, 100, "/dev/a", SessionState::Idle, Some(1), 1);
        let mut n = c("n", 2, 100, "/dev/a", SessionState::Idle, Some(2), 2);
        n.pid = None;
        assert!(superseded(&[o, n]).is_empty(), "n has no pid to match on either");
    }
}
