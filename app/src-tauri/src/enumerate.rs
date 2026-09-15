// Cold start and the periodic rescan: `claude agents --json`.
// Documented, keys observed 2.1.220: pid, cwd, kind, startedAt,
// sessionId, name. ADR-024 recorded no status key on 2.1.220; a `status`
// key has since been observed on the installed 2.1.270, with values
// validated by the CLI itself: "busy", "shell", "idle", "waiting" (busy
// and idle seen live). ADR-035 puts that key to use: it can colour a
// session the hooks have never heard from, or recolour one sitting in
// `unknown`, but it never overrides a colour a hook has already set
// (state.rs `Session::apply_scan_state`), except for ADR-036's
// tie-break: two consecutive scans in a row that disagree with a
// hook-set colour, with no hook event landing between them, recolour
// the session anyway. Liveness (whether the session's row survives at
// all) stays a separate question, answered by a held process handle
// where one exists (liveness.rs, registry.rs), not by this status
// string.
//
// Fetching shells out and can take a second, so it is split from
// registering: fetch without the registry lock, register with it. The
// JSON-shape logic is further split from the shelling out (`parse`) so
// it can be tested without a `claude` binary on PATH.
//
// `register` also prunes: a session bound from a previous run that this
// run does not report is dropped from the list (unless a hook has heard
// from it inside the grace window, or a held process handle proves it
// is still alive; see registry::prune_missing). That pruning is only
// sound because `fetch` failing returns `None` rather than `Some(vec![])`,
// so a dead or unparseable `claude` can never be mistaken for "nobody is
// running" and empty the whole list.

use std::collections::HashSet;

use serde_json::Value;

use crate::registry::Registry;

pub struct Row {
    pub id: String,
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
    /// The scan's own idea of liveness (ADR-035): "busy", "shell",
    /// "idle", or "waiting" on the versions observed so far. `None` when
    /// the key is absent (an older CLI) or not a string.
    pub status: Option<String>,
    /// `startedAt`, ms epoch (documented, 2.1.220). `None` on an older
    /// CLI or a non-numeric value; feeds `supersede::superseded`'s
    /// ordering through `Registry::note_started_at`, whose change report
    /// `register` folds into its own, since a start time alone can
    /// decide which rows the board shows.
    pub started_at_ms: Option<i64>,
}

pub fn fetch() -> Option<Vec<Row>> {
    let out = std::process::Command::new("claude")
        .args(["agents", "--json"])
        .output()
        .ok()?;
    // The exit code is deliberately ignored: on 2.1.220 the command
    // emits valid JSON and exits 255 (observed 2026-08-02). Parseable
    // output is the success signal here.
    parse(&out.stdout)
}

/// Turn `claude agents --json` stdout into rows. Accepts either a bare
/// array or an object wrapping one under any key, so a schema that grows
/// a wrapper does not kill cold start. A row missing a string
/// `sessionId` cannot be attributed to anything and is dropped rather
/// than failing the whole batch.
pub fn parse(stdout: &[u8]) -> Option<Vec<Row>> {
    let parsed: Value = serde_json::from_slice(stdout).ok()?;
    let rows = match parsed {
        Value::Array(rows) => rows,
        Value::Object(map) => map.into_iter().find_map(|(_, v)| match v {
            Value::Array(rows) => Some(rows),
            _ => None,
        })?,
        _ => return None,
    };
    Some(
        rows.iter()
            .filter_map(|row| {
                Some(Row {
                    id: row.get("sessionId")?.as_str()?.to_string(),
                    name: row.get("name").and_then(Value::as_str).map(String::from),
                    cwd: row.get("cwd").and_then(Value::as_str).map(String::from),
                    pid: row.get("pid").and_then(Value::as_u64).map(|p| p as u32),
                    status: row.get("status").and_then(Value::as_str).map(String::from),
                    started_at_ms: row.get("startedAt").and_then(Value::as_i64),
                })
            })
            .collect(),
    )
}

/// Register every row from a successful enumeration, then prune any
/// bound session this run does not report. Only call this with rows from
/// a run that actually succeeded (`fetch` returned `Some`, even if the
/// vec is empty): a failed run must never reach here, or an empty
/// `present` set would prune every session in the list.
pub fn register(reg: &mut Registry, rows: &[Row], now_ms: i64) -> bool {
    let mut changed = false;
    let mut present: HashSet<String> = HashSet::with_capacity(rows.len());
    for row in rows {
        present.insert(row.id.clone());
        changed |= reg.register_enumerated(
            &row.id,
            row.name.as_deref(),
            row.cwd.as_deref(),
            row.pid,
            row.status.as_deref(),
            now_ms,
        );
        changed |= reg.note_started_at(&row.id, row.started_at_ms);
    }
    changed |= reg.prune_missing(&present, now_ms);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_bare_array_parses() {
        let stdout = json!([{"sessionId": "s1"}]).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
    }

    #[test]
    fn an_object_wrapping_the_array_under_any_key_parses() {
        let stdout = json!({"agents": [{"sessionId": "s1"}]}).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
    }

    #[test]
    fn rows_missing_session_id_are_skipped() {
        let stdout = json!([{"cwd": "C:/dev/a"}, {"sessionId": "s1"}]).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
    }

    #[test]
    fn a_non_string_session_id_is_skipped() {
        let stdout = json!([{"sessionId": 5}, {"sessionId": "s1"}]).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
    }

    #[test]
    fn pid_as_u64_lands_as_u32() {
        let stdout = json!([{"sessionId": "s1", "pid": 4242}]).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows[0].pid, Some(4242u32));
    }

    #[test]
    fn status_is_parsed_when_present_as_a_string_and_none_otherwise() {
        let stdout = json!([
            {"sessionId": "s1", "status": "busy"},
            {"sessionId": "s2"},
            {"sessionId": "s3", "status": 5},
        ])
        .to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows[0].status.as_deref(), Some("busy"));
        assert_eq!(rows[1].status, None, "an older CLI with no status key must parse fine");
        assert_eq!(rows[2].status, None, "a non-string status must not be coerced");
    }

    #[test]
    fn started_at_is_parsed_when_present_as_a_number_and_none_otherwise() {
        let stdout = json!([
            {"sessionId": "s1", "startedAt": 1_789_492_719_945i64},
            {"sessionId": "s2"},
            {"sessionId": "s3", "startedAt": "not a number"},
        ])
        .to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        assert_eq!(rows[0].started_at_ms, Some(1_789_492_719_945));
        assert_eq!(rows[1].started_at_ms, None, "an older CLI with no startedAt key must parse fine");
        assert_eq!(rows[2].started_at_ms, None, "a non-numeric startedAt must not be coerced");
    }

    #[test]
    fn register_stores_started_at_on_the_session_for_supersede_to_read() {
        let stdout = json!([{"sessionId": "s1", "startedAt": 1_789_492_719_945i64}]).to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        let mut reg = Registry::default();
        register(&mut reg, &rows, 10);
        assert_eq!(reg.sessions["s1"].started_at_ms, Some(1_789_492_719_945));
    }

    #[test]
    fn register_reports_a_change_when_started_at_alone_flips_supersession() {
        // The 2026-09-15 review: two idle sessions in one VS Code window
        // and folder, discovered in the same scan with no `startedAt`,
        // tie on first-seen and both show. A later scan that differs
        // only in naming distinct start times hides the older one, and
        // the scan loop only repaints and resizes when `register` says
        // something changed, so that timestamp-only transition has to
        // count, and an identical scan after it must not.
        let rows = |started: bool| {
            let mut a = json!({"sessionId": "a", "name": "a", "cwd": "C:/dev/deckhand", "pid": 111, "status": "idle"});
            let mut b = json!({"sessionId": "b", "name": "b", "cwd": "C:/dev/deckhand", "pid": 222, "status": "idle"});
            if started {
                a["startedAt"] = json!(100);
                b["startedAt"] = json!(200);
            }
            parse(&json!([a, b]).to_string().into_bytes()).unwrap()
        };
        let mut reg = Registry::default();
        assert!(register(&mut reg, &rows(false), 10));
        // host::resolve walked the real process table for pids that are
        // not claude.exe; stand in the VS Code window facts directly, the
        // same way registry.rs's supersession tests do. The pids do not
        // change again below, so nothing re-resolves them.
        for id in ["a", "b"] {
            let s = reg.sessions.get_mut(id).unwrap();
            s.host = Some(crate::host::Host::VsCode);
            s.parent_pid = Some(4242);
        }
        assert_eq!(reg.snapshot(11).tiles.len(), 2, "tied on first-seen, neither is hidden");

        assert!(register(&mut reg, &rows(true), 20), "a startedAt-only change must report a change");
        assert_eq!(reg.snapshot(21).tiles.len(), 1, "with start times known, the older row is hidden");
        assert!(!register(&mut reg, &rows(true), 30), "an identical follow-up scan is a no-op");
    }

    #[test]
    fn empty_array_gives_some_empty() {
        let rows = parse(b"[]").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn invalid_json_gives_none() {
        assert!(parse(b"not json").is_none());
    }

    #[test]
    fn a_top_level_string_or_number_gives_none() {
        assert!(parse(b"\"hello\"").is_none());
        assert!(parse(b"42").is_none());
    }

    #[test]
    fn register_prunes_a_bound_session_a_successful_run_no_longer_reports() {
        let mut reg = Registry::default();
        // s1 was bound by an earlier successful run; this run reports
        // only s2, and s1 has not been heard from since (well past the
        // enumeration grace window).
        assert!(register(&mut reg, &parse(&json!([{"sessionId": "s1"}]).to_string().into_bytes()).unwrap(), 0));
        let later = crate::registry::ENUM_GRACE_MS + 1;
        let rows = parse(&json!([{"sessionId": "s2"}]).to_string().into_bytes()).unwrap();
        assert!(register(&mut reg, &rows, later));
        assert!(!reg.is_bound("s1"), "a session missing from a successful run must be pruned");
        assert!(reg.is_bound("s2"));
    }

    #[test]
    fn register_never_prunes_within_the_grace_window() {
        let mut reg = Registry::default();
        assert!(register(&mut reg, &parse(&json!([{"sessionId": "s1"}]).to_string().into_bytes()).unwrap(), 0));
        // A run moments later that no longer reports s1 must not drop it
        // yet: enumeration can lag a live hook by a beat or two.
        let rows = parse(&json!([{"sessionId": "s2"}]).to_string().into_bytes()).unwrap();
        register(&mut reg, &rows, 10);
        assert!(reg.is_bound("s1"), "a session inside the grace window must survive");
    }

    #[test]
    fn register_fills_id_label_cwd_and_pid_with_unknown_state() {
        let stdout =
            json!([{"sessionId": "s1", "name": "My Session", "cwd": "C:/dev/undertow", "pid": 555}])
                .to_string();
        let rows = parse(stdout.as_bytes()).unwrap();
        let mut reg = Registry::default();
        assert!(register(&mut reg, &rows, 10));
        let s = &reg.sessions["s1"];
        assert_eq!(s.label, "My Session", "a name wins over the cwd-derived label");
        assert_eq!(s.cwd.as_deref(), Some("C:/dev/undertow"));
        assert_eq!(s.pid, Some(555));
        assert_eq!(s.state, crate::state::SessionState::Unknown);
    }
}
