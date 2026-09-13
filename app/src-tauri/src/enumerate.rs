// Cold start: `claude agents --json`. Documented, keys observed 2.1.220:
// pid, cwd, kind, startedAt, sessionId, name. No status key exists on
// 2.1.220 (ADR-024), so this channel recovers bindings and labels, never
// state: every session it registers lands in unknown and stays there
// until a hook event colours it.
//
// Fetching shells out and can take a second, so it is split from
// registering: fetch without the registry lock, register with it. The
// JSON-shape logic is further split from the shelling out (`parse`) so
// it can be tested without a `claude` binary on PATH.

use serde_json::Value;

use crate::registry::Registry;

pub struct Row {
    pub id: String,
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
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
                })
            })
            .collect(),
    )
}

pub fn register(reg: &mut Registry, rows: &[Row], now_ms: i64) -> bool {
    let mut changed = false;
    for row in rows {
        changed |= reg.register_enumerated(
            &row.id,
            row.name.as_deref(),
            row.cwd.as_deref(),
            row.pid,
            now_ms,
        );
    }
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
