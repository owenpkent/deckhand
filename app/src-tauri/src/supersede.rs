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
    /// channel: the fallback ordering, for a whole window-and-folder
    /// group at once, when any session in it has no `startedAt`.
    pub first_seen_ms: i64,
}

/// True when `a` and `b` share one VS Code window and folder: both have
/// a known pid, both are classified `Host::VsCode`, both name the same
/// direct parent pid, and both name the same folder (normalized, so
/// case, separators, and a trailing slash do not split a match).
/// Reflexive on any candidate that qualifies at all, symmetric, and
/// transitive, which is what lets `superseded` treat "same window and
/// folder" as a partition into groups and order each group by one
/// clock.
fn same_window_and_folder(a: &Candidate, b: &Candidate) -> bool {
    if a.pid.is_none() || b.pid.is_none() {
        return false;
    }
    if a.host != Some(Host::VsCode) || b.host != Some(Host::VsCode) {
        return false;
    }
    let (Some(a_parent), Some(b_parent)) = (a.parent_pid, b.parent_pid) else {
        return false;
    };
    if a_parent != b_parent {
        return false;
    }
    let (Some(a_cwd), Some(b_cwd)) = (a.cwd, b.cwd) else {
        return false;
    };
    crate::reveal::normalize_path(a_cwd) == crate::reveal::normalize_path(b_cwd)
}

/// Which clock orders one window-and-folder group: the scan's own
/// `startedAt` when every member reports one, first-seen otherwise.
/// Chosen once per group, never per pair. Choosing per pair, with each
/// pair falling back on its own, made "newer than" non-transitive as
/// soon as one member lacked `startedAt` (2026-09-15 review): A could
/// be newer than C by first-seen, C newer than B by first-seen, and B
/// newer than A by `startedAt`, a cycle that hid all three. One clock
/// per group is a total order, so the member it ranks newest can never
/// be hidden and a group never disappears entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clock {
    StartedAt,
    FirstSeen,
}

fn clock_for(group: &[&Candidate]) -> Clock {
    if group.iter().all(|c| c.started_at_ms.is_some()) {
        Clock::StartedAt
    } else {
        Clock::FirstSeen
    }
}

fn started(c: &Candidate, clock: Clock) -> i64 {
    match clock {
        // `StartedAt` is only ever chosen for a group whose every member
        // has one; the fallback here is unreachable by construction and
        // kept in place of a panic path.
        Clock::StartedAt => c.started_at_ms.unwrap_or(c.first_seen_ms),
        Clock::FirstSeen => c.first_seen_ms,
    }
}

/// True when `o` is superseded by `n`: same VS Code window and folder
/// (`same_window_and_folder`), `n` started after `o` on the group's
/// `clock`, and `o` sits in a state it is safe to hide (never
/// `Thinking`, `NeedsInput`, or `Error` -- a session doing something or
/// waiting on a human is never a stale duplicate). `n` itself may be in
/// any state except `Ended`: an ended session has already handed
/// nothing back, so it supersedes nothing. A tie on the clock hides
/// neither.
fn is_superseded_by(o: &Candidate, n: &Candidate, clock: Clock) -> bool {
    if o.id == n.id {
        return false;
    }
    if !same_window_and_folder(o, n) {
        return false;
    }
    if !matches!(o.state, SessionState::Idle | SessionState::Complete | SessionState::Unknown) {
        return false;
    }
    if n.state == SessionState::Ended {
        return false;
    }
    started(n, clock) > started(o, clock)
}

/// Every id in `candidates` that some other, newer candidate in its own
/// window-and-folder group makes safe to hide right now. Each
/// candidate's group is gathered fresh (candidate counts are a handful
/// of sessions at most, never enough for the O(n^2) scan to matter),
/// the group's clock is chosen once (`Clock`), and the pairwise check
/// runs on that clock alone. Pure: the caller decides what "hide" means
/// for its own list (registry.rs keeps the binding, only leaves the id
/// out of the tiles/row-count it builds).
pub fn superseded(candidates: &[Candidate]) -> HashSet<String> {
    let mut hidden = HashSet::new();
    for o in candidates {
        // `o`'s own group, `o` included when it qualifies at all; empty
        // when it does not (no pid, not VS Code, ...), which hides
        // nothing.
        let group: Vec<&Candidate> = candidates.iter().filter(|c| same_window_and_folder(o, c)).collect();
        let clock = clock_for(&group);
        if group.iter().any(|n| is_superseded_by(o, n, clock)) {
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
    fn started_at_is_only_used_when_every_session_in_the_group_reports_it() {
        // "older" has no startedAt at all; "newer" does, and it names a
        // moment earlier than "older"'s own first-seen time. If
        // startedAt leaked in from one side alone, this would flip the
        // ordering; instead the whole group falls back to first-seen,
        // so "older" (first-seen 10) is still the one hidden, by
        // "newer" (first-seen 20).
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

    #[test]
    fn mixed_started_at_availability_never_hides_a_whole_group() {
        // The 2026-09-15 review's counterexample: three idle sessions in
        // one window and folder, where A was discovered late (a large
        // first-seen) and C's row has no startedAt. Pair by pair, with
        // each pair picking its own clock, B beat A on startedAt, C beat
        // B on first-seen, and A beat C on first-seen: a cycle, and all
        // three rows vanished from a board that still had three live
        // sessions. With one clock per group (first-seen here, since C
        // has no startedAt), A is newest and stays, whatever order the
        // registry happens to hand the candidates over in.
        let a = c("a", 1, 100, "/dev/a", SessionState::Idle, Some(100), 500);
        let b = c("b", 2, 100, "/dev/a", SessionState::Idle, Some(200), 300);
        let cc = c("c", 3, 100, "/dev/a", SessionState::Idle, None, 400);
        let permutations = [[a, b, cc], [a, cc, b], [b, a, cc], [b, cc, a], [cc, a, b], [cc, b, a]];
        for order in permutations {
            let hidden = superseded(&order);
            assert!(hidden.len() < 3, "a group must never be hidden in its entirety: {hidden:?}");
            assert_eq!(ids(hidden), vec!["b".to_string(), "c".to_string()], "one clock, one answer, in any input order");
        }
    }

    #[test]
    fn the_clock_is_chosen_per_group_not_across_the_whole_list() {
        // Window 100: both report startedAt, and it disagrees with
        // first-seen (p was heard of first but started later), so
        // startedAt must decide and hide q. Window 999: r has no
        // startedAt, so that group, and only that group, falls back to
        // first-seen and hides r. A single clock for the whole list
        // would get one of the two wrong.
        let p = c("p", 1, 100, "/dev/a", SessionState::Idle, Some(200), 10);
        let q = c("q", 2, 100, "/dev/a", SessionState::Idle, Some(100), 20);
        let r = c("r", 3, 999, "/dev/a", SessionState::Idle, None, 30);
        let s = c("s", 4, 999, "/dev/a", SessionState::Idle, Some(5), 40);
        assert_eq!(ids(superseded(&[p, q, r, s])), vec!["q".to_string(), "r".to_string()]);
    }
}
