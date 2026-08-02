// Cold start: `claude agents --json`. Documented, keys observed 2.1.220:
// pid, cwd, kind, startedAt, sessionId, name. No status key exists on
// 2.1.220 (ADR-024), so this channel recovers bindings and labels, never
// state: every session it registers lands in unknown and stays there
// until a hook event colours it.
//
// Fetching shells out and can take a second, so it is split from
// registering: fetch without the registry lock, register with it.

use serde_json::Value;

use crate::registry::Registry;

pub struct Row {
    pub id: String,
    pub name: Option<String>,
    pub cwd: Option<String>,
}

pub fn fetch() -> Option<Vec<Row>> {
    let out = std::process::Command::new("claude")
        .args(["agents", "--json"])
        .output()
        .ok()?;
    // The exit code is deliberately ignored: on 2.1.220 the command
    // emits valid JSON and exits 255 (observed 2026-08-02). Parseable
    // output is the success signal here.
    let parsed: Value = serde_json::from_slice(&out.stdout).ok()?;
    // Accept either a bare array or an object wrapping one, so a schema
    // that grows a wrapper does not kill cold start.
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
            now_ms,
        );
    }
    changed
}
