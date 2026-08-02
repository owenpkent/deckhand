// The session state machine. One instance per session; this is the only
// place a status colour is decided (docs/ARCHITECTURE.md).
//
// Transitions implement the status-inference table in
// docs/CLAUDE_CODE_ADAPTER.md#status-inference and the daemon rules in
// docs/ARCHITECTURE.md (child ledger, operation bracketing, T_unknown).
// The tests at the bottom are that table, row by row. If a test here and
// the docs disagree, the docs win and this file is the bug.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Thinking,
    NeedsInput,
    Complete,
    Error,
    Ended,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    // Constructed by the Phase 2 gate; in the protocol from day one so
    // the surface never learns a second amber shape later.
    #[allow(dead_code)]
    Permission,
    Question,
}

/// An open operation for the liveness bracket: opened by `PreToolUse` or
/// `SubagentStart`, closed by its partner. While any is open the tile
/// shows elapsed-in-operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOp {
    pub id: Option<String>, // tool_use_id where the payload carries one
    pub tool: String,
    pub opened_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub kind: String,
    pub message: Option<String>,
}

/// `T_unknown`: one deadline, measured from the last event of any kind,
/// suspended by nothing (docs/ARCHITECTURE.md#liveness-by-open-operation).
pub const T_UNKNOWN_MS: i64 = 900_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub label: String,
    pub cwd: Option<String>,
    /// Rendered as text on the tile badge; "unknown" when absent, which
    /// is not rare. Never a colour (docs/UI_SPEC.md#corner-badges).
    pub permission_mode: Option<String>,
    /// From the enumeration where known; hooks do not carry one. Feeds
    /// Reveal's window match, nothing else.
    pub pid: Option<u32>,
    pub state: SessionState,
    pub state_since_ms: i64,
    pub detail_kind: Option<InputKind>,
    pub detail_tool: Option<String>,
    pub question: Option<String>,
    pub options: Vec<String>,
    pub error: Option<ErrorDetail>,
    /// The child ledger: open subagents only. Background Bash tasks emit
    /// no hook and are invisible to it; the count is a floor.
    pub children: u32,
    /// The ledger's identity backing: `agent_id` per open child
    /// (observed 2.1.220 on both subagent events). Keying on identity is
    /// what makes duplicate delivery a no-op, which adapter rule 4
    /// requires and a bare counter cannot provide.
    #[serde(skip)]
    child_ids: Vec<String>,
    pub open_ops: Vec<OpenOp>,
    pub last_event_at_ms: i64,
    pub unread_since_ms: Option<i64>,
    /// A turn ended while children were live; green arrives when the
    /// ledger empties (docs/UI_SPEC.md#the-child-ledger-and-complete).
    pub pending_complete: bool,
}

impl Session {
    pub fn new(id: String, now_ms: i64) -> Self {
        Session {
            id,
            label: String::new(),
            cwd: None,
            pid: None,
            permission_mode: None,
            // A session first seen by enumeration rather than by an
            // event starts here. Never idle: idle is the one guess that
            // looks like knowledge.
            state: SessionState::Unknown,
            state_since_ms: now_ms,
            detail_kind: None,
            detail_tool: None,
            question: None,
            options: Vec::new(),
            error: None,
            children: 0,
            child_ids: Vec::new(),
            open_ops: Vec::new(),
            last_event_at_ms: now_ms,
            unread_since_ms: None,
            pending_complete: false,
        }
    }

    fn set_state(&mut self, s: SessionState, now_ms: i64) {
        if self.state != s {
            self.state = s;
            self.state_since_ms = now_ms;
        }
        if s != SessionState::NeedsInput {
            self.detail_kind = None;
            self.question = None;
            self.options.clear();
        }
        if s != SessionState::Complete {
            self.unread_since_ms = None;
        }
    }

    fn open_op(&mut self, id: Option<String>, tool: &str, now_ms: i64) {
        self.open_ops.push(OpenOp {
            id,
            tool: tool.to_string(),
            opened_at_ms: now_ms,
        });
    }

    /// Close by `tool_use_id` when the payload carries one, else by tool
    /// name newest-first, else the newest open operation. A bracket that
    /// never closes is the same defect as a wrong colour, arriving more
    /// slowly, so the fallbacks err toward closing.
    fn close_op(&mut self, id: Option<&str>, tool: Option<&str>) {
        let idx = match id {
            Some(id) => self.open_ops.iter().rposition(|o| o.id.as_deref() == Some(id)),
            None => None,
        }
        .or_else(|| match tool {
            Some(t) => self.open_ops.iter().rposition(|o| o.tool == t),
            None => None,
        })
        .or_else(|| if self.open_ops.is_empty() { None } else { Some(self.open_ops.len() - 1) });
        if let Some(i) = idx {
            self.open_ops.remove(i);
        }
    }

    /// Apply one hook event. Returns true if anything observable changed.
    /// Unrecognised events update liveness and nothing else: the daemon
    /// takes no state change from an event it does not understand.
    pub fn apply_hook(&mut self, payload: &Value, now_ms: i64) -> bool {
        self.last_event_at_ms = now_ms;

        if let Some(m) = payload.get("permission_mode").and_then(Value::as_str) {
            self.permission_mode = Some(m.to_string());
        }
        if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
            if self.cwd.is_none() {
                self.cwd = Some(cwd.to_string());
            }
            if self.label.is_empty() {
                self.label = dir_name(cwd);
            }
        }

        let event = payload
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or("");
        let tool_name = payload.get("tool_name").and_then(Value::as_str);
        let tool_use_id = payload.get("tool_use_id").and_then(Value::as_str);

        match event {
            "SessionStart" => {
                let source = payload.get("source").and_then(Value::as_str).unwrap_or("");
                if source == "compact" {
                    // Auto-compaction fires mid-turn; mapping it to idle
                    // would flip a live blue tile white.
                    return false;
                }
                self.children = 0;
                self.child_ids.clear();
                self.open_ops.clear();
                self.pending_complete = false;
                self.error = None;
                self.detail_tool = None;
                self.set_state(SessionState::Idle, now_ms);
                true
            }
            "UserPromptSubmit" => {
                self.pending_complete = false;
                self.error = None;
                self.set_state(SessionState::Thinking, now_ms);
                true
            }
            "PreToolUse" => {
                let tool = tool_name.unwrap_or("(unknown tool)");
                self.open_op(tool_use_id.map(String::from), tool, now_ms);
                if tool == "AskUserQuestion" {
                    let (q, opts) = parse_question(payload.get("tool_input"));
                    self.set_state(SessionState::NeedsInput, now_ms);
                    self.detail_kind = Some(InputKind::Question);
                    self.question = q;
                    self.options = opts;
                } else {
                    // Phase 1 installs no gate, so the held-by-Deckhand
                    // row of the table cannot occur yet. When the gate
                    // arrives (Phase 2) it lands here, above this arm.
                    self.set_state(SessionState::Thinking, now_ms);
                    self.detail_tool = Some(tool.to_string());
                }
                true
            }
            "PostToolUse" => {
                self.close_op(tool_use_id, tool_name);
                self.set_state(SessionState::Thinking, now_ms);
                true
            }
            "PostToolUseFailure" => {
                // Observed 2.1.220: `error` is a string carrying the
                // tool's own output and `is_interrupt` marks an
                // interrupted call. There is no `error_type` field; the
                // documented shape said otherwise and reality won.
                let is_interrupt = payload
                    .get("is_interrupt")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if is_interrupt {
                    // An interrupt closes every operation open on the
                    // session, not just this call's.
                    self.open_ops.clear();
                } else {
                    self.close_op(tool_use_id, tool_name);
                }
                self.error = Some(ErrorDetail {
                    kind: if is_interrupt { "interrupt" } else { "tool_failure" }.to_string(),
                    message: payload
                        .get("error")
                        .and_then(Value::as_str)
                        .map(|s| truncate(s, 200)),
                });
                // The turn continues: a failed tool call is not a failed
                // turn.
                self.set_state(SessionState::Thinking, now_ms);
                true
            }
            "PermissionDenied" => {
                // "A call was refused by something that is not you." It
                // keeps the tile blue and closes the bracket, no more.
                self.close_op(tool_use_id, tool_name);
                self.set_state(SessionState::Thinking, now_ms);
                true
            }
            "Notification" => {
                match payload
                    .get("notification_type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                {
                    "agent_needs_input" => {
                        // No kind: the payload does not say which sort of
                        // prompt it is, so neither Approve nor Answer may
                        // light on this amber.
                        self.set_state(SessionState::NeedsInput, now_ms);
                        self.detail_kind = None;
                        true
                    }
                    "agent_completed" => {
                        self.set_state(SessionState::Idle, now_ms);
                        true
                    }
                    _ => false,
                }
            }
            "Stop" => {
                if self.children > 0 {
                    // Green while work continues would falsify the one
                    // promise the board makes.
                    self.pending_complete = true;
                    self.set_state(SessionState::Thinking, now_ms);
                } else {
                    self.set_state(SessionState::Complete, now_ms);
                    self.unread_since_ms = Some(now_ms);
                }
                true
            }
            "StopFailure" => {
                let kind = match payload.get("error") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Object(o)) => o
                        .get("type")
                        .or_else(|| o.get("kind"))
                        .and_then(Value::as_str)
                        .unwrap_or("api_error")
                        .to_string(),
                    _ => "api_error".to_string(),
                };
                self.error = Some(ErrorDetail { kind, message: None });
                self.set_state(SessionState::Error, now_ms);
                true
            }
            "SubagentStart" => {
                // Observed 2.1.220: carries `agent_id` and `agent_type`.
                // The ledger keys on identity so a duplicate delivery
                // changes nothing, and the start opens an operation for
                // the liveness bracket per the table in
                // docs/ARCHITECTURE.md#liveness-by-open-operation.
                let agent_id = payload
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if agent_id.is_empty() || !self.child_ids.contains(&agent_id) {
                    let agent_type = payload
                        .get("agent_type")
                        .and_then(Value::as_str)
                        .unwrap_or("subagent");
                    self.child_ids.push(agent_id.clone());
                    self.open_op(Some(agent_id), agent_type, now_ms);
                    self.children = self.child_ids.len() as u32;
                }
                true // the badge changed even though the colour did not
            }
            "SubagentStop" => {
                let agent_id = payload.get("agent_id").and_then(Value::as_str);
                match agent_id.and_then(|id| self.child_ids.iter().position(|c| c == id)) {
                    Some(i) => {
                        let id = self.child_ids.remove(i);
                        self.close_op(Some(&id), None);
                    }
                    None => {
                        // A stop for a child never seen. Shrink the
                        // ledger anyway rather than pin the count high
                        // forever, but close no unrelated operation.
                        if !self.child_ids.is_empty() {
                            let id = self.child_ids.pop().unwrap_or_default();
                            self.close_op(Some(&id), None);
                        }
                    }
                }
                self.children = self.child_ids.len() as u32;
                if self.children == 0 && self.pending_complete {
                    self.pending_complete = false;
                    self.set_state(SessionState::Complete, now_ms);
                    self.unread_since_ms = Some(now_ms);
                }
                true
            }
            "SessionEnd" => {
                let reason = payload.get("reason").and_then(Value::as_str).unwrap_or("");
                if reason == "clear" || reason == "resume" {
                    // Each is followed by a new SessionStart under a
                    // session that never stopped.
                    return false;
                }
                self.children = 0;
                self.child_ids.clear();
                self.open_ops.clear();
                self.pending_complete = false;
                self.set_state(SessionState::Ended, now_ms);
                true
            }
            _ => false,
        }
    }

    /// The surface selected this tile. Green clears to idle; an error is
    /// acknowledged the same way. (Crash detection, where an acknowledged
    /// red would become ended, is an open question and not implemented.)
    pub fn on_selected(&mut self, now_ms: i64) -> bool {
        match self.state {
            SessionState::Complete | SessionState::Error => {
                self.error = None;
                self.set_state(SessionState::Idle, now_ms);
                true
            }
            _ => false,
        }
    }

    /// The `T_unknown` deadline. Returns true if the session moved.
    pub fn tick(&mut self, now_ms: i64) -> bool {
        if self.state == SessionState::Ended || self.state == SessionState::Unknown {
            return false;
        }
        if now_ms - self.last_event_at_ms > T_UNKNOWN_MS {
            // Never to error: a long tool call is normal, and a wrong red
            // costs more than an honest grey.
            self.set_state(SessionState::Unknown, now_ms);
            return true;
        }
        false
    }
}

/// Clip detail text to what a tile or panel can honestly show. Real
/// `error` strings carry whole compiler dumps.
fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut cut = max;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}...", &text[..cut])
}

pub fn dir_name(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

/// Pull the first question and its option labels out of an
/// `AskUserQuestion` tool input. The shape is `documented`, not
/// `observed`, so every step is defensive: absent pieces produce an
/// amber with no options rather than a parse failure.
fn parse_question(tool_input: Option<&Value>) -> (Option<String>, Vec<String>) {
    let Some(input) = tool_input else {
        return (None, Vec::new());
    };
    let first = input
        .get("questions")
        .and_then(Value::as_array)
        .and_then(|a| a.first());
    let Some(q) = first else {
        return (None, Vec::new());
    };
    let text = q.get("question").and_then(Value::as_str).map(String::from);
    let options = q
        .get("options")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    o.get("label")
                        .and_then(Value::as_str)
                        .or_else(|| o.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    (text, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn s() -> Session {
        Session::new("test".into(), 0)
    }

    fn ev(session: &mut Session, now: i64, payload: Value) -> bool {
        session.apply_hook(&payload, now)
    }

    #[test]
    fn session_start_registers_and_goes_idle() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionStart", "source": "startup", "cwd": "C:\\Users\\o\\dev\\undertow"}));
        assert_eq!(x.state, SessionState::Idle);
        assert_eq!(x.label, "undertow");
    }

    #[test]
    fn compaction_is_not_a_lifecycle_change() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        ev(&mut x, 2, json!({"hook_event_name": "SessionStart", "source": "compact"}));
        assert_eq!(x.state, SessionState::Thinking, "compaction must not flip a live blue tile white");
    }

    #[test]
    fn clear_and_resume_are_not_endings() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        ev(&mut x, 2, json!({"hook_event_name": "SessionEnd", "reason": "clear"}));
        assert_eq!(x.state, SessionState::Thinking);
        ev(&mut x, 3, json!({"hook_event_name": "SessionEnd", "reason": "resume"}));
        assert_eq!(x.state, SessionState::Thinking);
        ev(&mut x, 4, json!({"hook_event_name": "SessionEnd", "reason": "exit"}));
        assert_eq!(x.state, SessionState::Ended);
    }

    #[test]
    fn ask_user_question_is_amber_with_kind_question() {
        let mut x = s();
        ev(&mut x, 1, json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "AskUserQuestion",
            "tool_use_id": "t1",
            "tool_input": {"questions": [{"question": "Which?", "options": [{"label": "Full option A"}, {"label": "Full option B"}]}]}
        }));
        assert_eq!(x.state, SessionState::NeedsInput);
        assert_eq!(x.detail_kind, Some(InputKind::Question));
        assert_eq!(x.question.as_deref(), Some("Which?"));
        assert_eq!(x.options, vec!["Full option A", "Full option B"]);
        assert_eq!(x.open_ops.len(), 1, "every PreToolUse opens an operation");
    }

    #[test]
    fn ordinary_tool_call_is_thinking_and_brackets() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        assert_eq!(x.state, SessionState::Thinking);
        assert_eq!(x.detail_tool.as_deref(), Some("Bash"));
        assert_eq!(x.open_ops.len(), 1);
        ev(&mut x, 2, json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        assert!(x.open_ops.is_empty(), "PostToolUse closes the operation");
        assert_eq!(x.state, SessionState::Thinking);
    }

    #[test]
    fn tool_failure_keeps_the_turn_alive() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        // The observed shape: `error` is a string, `is_interrupt` a bool.
        ev(&mut x, 2, json!({"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_use_id": "t1", "error": "Exit code 101\nlots of compiler output", "is_interrupt": false}));
        assert_eq!(x.state, SessionState::Thinking, "a failed tool call is not a failed turn");
        assert!(x.open_ops.is_empty());
        assert_eq!(x.error.as_ref().unwrap().kind, "tool_failure");
        assert!(x.error.as_ref().unwrap().message.as_ref().unwrap().starts_with("Exit code 101"));
    }

    #[test]
    fn an_interrupt_closes_every_open_operation() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 2, json!({"hook_event_name": "PreToolUse", "tool_name": "Read", "tool_use_id": "t2"}));
        assert_eq!(x.open_ops.len(), 2);
        ev(&mut x, 3, json!({"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_use_id": "t1", "error": "interrupted", "is_interrupt": true}));
        assert!(x.open_ops.is_empty(), "an interrupt closes every open operation, not just its own");
        assert_eq!(x.error.as_ref().unwrap().kind, "interrupt");
    }

    #[test]
    fn error_detail_is_truncated_to_panel_size() {
        let mut x = s();
        let long = "x".repeat(5000);
        ev(&mut x, 1, json!({"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "error": long, "is_interrupt": false}));
        assert!(x.error.as_ref().unwrap().message.as_ref().unwrap().len() <= 203);
    }

    #[test]
    fn permission_denied_keeps_blue_and_closes_the_bracket() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 2, json!({"hook_event_name": "PermissionDenied", "tool_use_id": "t1", "reason": "Blocked by classifier"}));
        assert_eq!(x.state, SessionState::Thinking);
        assert!(x.open_ops.is_empty(), "a denial ends the operation");
    }

    #[test]
    fn notification_needs_input_carries_no_kind() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Notification", "notification_type": "agent_needs_input"}));
        assert_eq!(x.state, SessionState::NeedsInput);
        assert_eq!(x.detail_kind, None, "no guessed kind: neither Approve nor Answer may light");
        ev(&mut x, 2, json!({"hook_event_name": "Notification", "notification_type": "agent_completed"}));
        assert_eq!(x.state, SessionState::Idle);
        assert!(!ev(&mut x, 3, json!({"hook_event_name": "Notification", "notification_type": "something_new"})));
    }

    #[test]
    fn stop_is_green_only_with_an_empty_ledger() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SubagentStart"}));
        assert_eq!(x.children, 1);
        ev(&mut x, 2, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state, SessionState::Thinking, "complete is unreachable while the ledger is non-empty");
        ev(&mut x, 3, json!({"hook_event_name": "SubagentStop"}));
        assert_eq!(x.state, SessionState::Complete, "green arrives when the ledger empties");
        assert!(x.unread_since_ms.is_some());
    }

    #[test]
    fn duplicate_subagent_start_is_idempotent() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "Explore"}));
        ev(&mut x, 2, json!({"hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "Explore"}));
        assert_eq!(x.children, 1, "the same update delivered twice must not change anything");
        ev(&mut x, 3, json!({"hook_event_name": "SubagentStop", "agent_id": "a1"}));
        assert_eq!(x.children, 0);
        assert!(x.open_ops.is_empty(), "the subagent bracket closed by agent_id");
    }

    #[test]
    fn subagent_stop_never_closes_an_unrelated_bracket() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 2, json!({"hook_event_name": "SubagentStop", "agent_id": "ghost"}));
        assert_eq!(x.open_ops.len(), 1, "a stop for an unseen child must not close a tool bracket");
    }

    #[test]
    fn children_never_move_the_parent_colour() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        ev(&mut x, 2, json!({"hook_event_name": "SubagentStart"}));
        assert_eq!(x.state, SessionState::Thinking);
        ev(&mut x, 3, json!({"hook_event_name": "SubagentStop"}));
        assert_eq!(x.state, SessionState::Thinking);
    }

    #[test]
    fn stop_failure_is_red_with_a_typed_error() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "StopFailure", "error": {"type": "api_error"}}));
        assert_eq!(x.state, SessionState::Error);
        assert_eq!(x.error.as_ref().unwrap().kind, "api_error");
    }

    #[test]
    fn selection_clears_green_to_white() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state, SessionState::Complete);
        assert!(x.on_selected(2));
        assert_eq!(x.state, SessionState::Idle);
        assert!(x.unread_since_ms.is_none());
    }

    #[test]
    fn t_unknown_greys_and_never_reds() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(!x.tick(T_UNKNOWN_MS));
        assert!(x.tick(T_UNKNOWN_MS + 2));
        assert_eq!(x.state, SessionState::Unknown);
        // Any authoritative event leaves unknown.
        ev(&mut x, T_UNKNOWN_MS + 3, json!({"hook_event_name": "UserPromptSubmit"}));
        assert_eq!(x.state, SessionState::Thinking);
    }

    #[test]
    fn a_session_first_seen_by_enumeration_is_unknown_not_idle() {
        let x = Session::new("enumerated".into(), 5);
        assert_eq!(x.state, SessionState::Unknown, "never guess idle");
    }

    #[test]
    fn unrecognised_events_update_liveness_only() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        let changed = ev(&mut x, 2, json!({"hook_event_name": "SomeFutureEvent"}));
        assert!(!changed);
        assert_eq!(x.state, SessionState::Thinking);
        assert_eq!(x.last_event_at_ms, 2, "liveness still updates");
    }

    #[test]
    fn permission_mode_travels_on_any_payload() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit", "permission_mode": "auto"}));
        assert_eq!(x.permission_mode.as_deref(), Some("auto"));
    }
}
