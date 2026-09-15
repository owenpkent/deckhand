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

/// `T_unknown`: one deadline, suspended by nothing
/// (docs/ARCHITECTURE.md#liveness-by-open-operation). What it is measured
/// from depends on whether the registry holds a live process handle for
/// the session (ADR-035, `Session::tick`): without one, the last event of
/// any kind, a hook or a scan sighting alike; with one, only the one case
/// a live process cannot itself vouch for, hooks silent past this
/// deadline while the scan does not confirm the turn is still busy.
pub const T_UNKNOWN_MS: i64 = 900_000;

/// The number of consecutive scans that must contradict a hook-set
/// colour, with no hook event landing between them, before the scan is
/// allowed to recolour the session anyway (ADR-036, `apply_scan_state`'s
/// tie-break). Two scans is about thirty seconds at the 15 s rescan
/// (`RESCAN_INTERVAL`, main.rs).
pub const SCAN_TIEBREAK_SCANS: u32 = 2;

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
    /// The last time a successful scan (`claude agents --json`) listed
    /// this session, set by `note_seen`. A scan sighting counts as an
    /// event of any kind for the not-alive half of `tick` (ADR-035),
    /// even though it is not a hook and never touches `heard`.
    #[serde(skip)]
    pub last_seen_ms: i64,
    /// The raw `status` string from the latest scan that listed this
    /// session ("busy", "shell", "idle", "waiting", or unrecognised),
    /// set by `note_seen`. What `tick` checks, while the process is
    /// alive, before greying a `Thinking` session the hooks have gone
    /// quiet on (ADR-035).
    #[serde(skip)]
    pub scan_status: Option<String>,
    /// True once any hook event has arrived for this session in this
    /// run. Separates the two roads to unknown for the surface's state
    /// word: bound by enumeration or restored from disk and never heard
    /// from, versus heard from and then silent past `T_unknown`. Neither
    /// is a guess, and neither changes the colour. Also the gate on
    /// `apply_scan_state` (ADR-035): once a hook has coloured a session,
    /// only a hook may recolour it; the scan may still colour one that
    /// has never been heard from, or one sitting in `unknown`. ADR-036
    /// carves out one exception: two consecutive scans that contradict
    /// the hook-set colour, with no hook event landing between them,
    /// recolour the session anyway (`scan_disagreements`).
    pub heard: bool,
    pub unread_since_ms: Option<i64>,
    /// A turn ended while children were live; green arrives when the
    /// ledger empties (docs/UI_SPEC.md#the-child-ledger-and-complete).
    pub pending_complete: bool,
    /// Consecutive scans, with no hook event in between, that have
    /// contradicted a hook-set colour (ADR-036). Reset to 0 by every
    /// hook event (`apply_hook`) and by `apply_scan_state` whenever a
    /// scan agrees, or does not count as a disagreement at all; counted
    /// up by a disagreeing scan until it reaches `SCAN_TIEBREAK_SCANS`,
    /// at which point the scan's colour wins and the counter resets.
    #[serde(skip)]
    pub scan_disagreements: u32,
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
            last_seen_ms: 0,
            scan_status: None,
            heard: false,
            unread_since_ms: None,
            pending_complete: false,
            scan_disagreements: 0,
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
        let event = payload
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or("");

        // Once a session has ended, only a session-start event (a resume)
        // may revive it. Every other event is ignored outright, not
        // merely processed to no effect: a straggler delivered late, or
        // racing SessionEnd itself (a Stop, a PostToolUseFailure, ...),
        // would otherwise flip Ended back to Complete/Thinking/Error.
        // Registry::apply_hook decides list membership from the state
        // left here after this call returns, so a straggler that got
        // through this guard would re-list a session that has already
        // left (docs/ARCHITECTURE.md#the-session-state-machine).
        if self.state == SessionState::Ended && event != "SessionStart" {
            return false;
        }

        self.last_event_at_ms = now_ms;
        self.heard = true;
        // Any event, recognised or not, is the hook channel speaking;
        // ADR-036's tie-break only fires on scans with no hook between
        // them, so any hook event clears the count.
        self.scan_disagreements = 0;

        if let Some(m) = payload.get("permission_mode").and_then(Value::as_str) {
            self.permission_mode = Some(m.to_string());
        }
        // A payload carrying `agent_id` is a subagent's, not the
        // session's own: subagent hook payloads share the parent
        // session_id but can carry a different cwd (their own working
        // directory), which would otherwise latch onto the session and
        // poison every later Reveal (docs/CONTROL_MAPPING.md). Liveness
        // above still updates; only cwd and the label derived from it
        // are skipped.
        let is_subagent_payload = payload.get("agent_id").is_some();
        if !is_subagent_payload {
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                if self.cwd.is_none() {
                    self.cwd = Some(cwd.to_string());
                }
                if self.label.is_empty() {
                    self.label = dir_name(cwd);
                }
            }
        }

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
                self.end_session(now_ms);
                true
            }
            _ => false,
        }
    }

    /// The lifecycle-ending move shared by `SessionEnd` (above) and
    /// `process_exited` (below): clears the child ledger and every open
    /// operation, and the pid (the OS is free to reuse it for an
    /// unrelated process the moment this one exits, so it must not
    /// survive to name Reveal's target for whatever comes next under the
    /// same session id), then sets `Ended`. Callers decide whether the
    /// move is legal (SessionEnd's own `clear`/`resume` guard;
    /// `process_exited`'s already-ended no-op).
    fn end_session(&mut self, now_ms: i64) {
        self.children = 0;
        self.child_ids.clear();
        self.open_ops.clear();
        self.pending_complete = false;
        self.pid = None;
        self.set_state(SessionState::Ended, now_ms);
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

    /// True for the scan status values that mean a turn is in progress
    /// well enough that hook silence should not be read as trouble
    /// (ADR-035): "busy" and "shell" are both mid-turn, and "waiting" is
    /// the scan's own amber, not silence at all.
    fn scan_says_in_flight(&self) -> bool {
        matches!(self.scan_status.as_deref(), Some("busy") | Some("shell") | Some("waiting"))
    }

    /// Record that a successful scan listed this session, independent of
    /// whether its status maps to a colour change. Never touches `heard`
    /// or the state machine itself; `apply_scan_state` is the half that
    /// can (ADR-035).
    pub fn note_seen(&mut self, status: Option<&str>, now_ms: i64) {
        self.last_seen_ms = now_ms;
        self.scan_status = status.map(String::from);
    }

    /// Colour a session from the scan's own status, the other half of
    /// ADR-035: "busy" and "shell" both read as a turn in flight
    /// (`Thinking`; the scan does not distinguish a shell command from
    /// any other tool call), "waiting" as `NeedsInput`, "idle" as
    /// `Idle`, and anything else, including no status at all, changes
    /// nothing. Ended sessions are never touched.
    ///
    /// Applied immediately when a hook has never coloured this session
    /// (`!self.heard`) or it is presently `unknown`: a hook carries
    /// detail the scan cannot (which tool, which question, which error)
    /// and always owns the colour once it has spoken.
    ///
    /// ADR-036's tie-break is the one exception, for a heard, coloured
    /// session: `Thinking` versus a scan that says `Idle`, or `Idle`,
    /// `Complete` or `Error` versus a scan that says `Thinking` (a lost
    /// `UserPromptSubmit`), counts as a disagreement. `NeedsInput` in any
    /// case, a `waiting` status, an absent or unrecognised status, and
    /// plain agreement never count, and reset the count to 0. Once
    /// `scan_disagreements` reaches `SCAN_TIEBREAK_SCANS` with no hook
    /// event landing in between, the scan's coarse colour wins: the
    /// target `Idle` mirrors the interrupt path of `PostToolUseFailure`
    /// plus clearing the child ledger (a session the CLI calls idle has
    /// nothing in flight), and the target `Thinking` mirrors
    /// `UserPromptSubmit` (a new turn whose own event was lost). This
    /// path never produces `Complete` or `NeedsInput`.
    ///
    /// Never sets `heard`, `question`, `options`, `detail_kind`, or
    /// `detail_tool`; touches `error` only where the mirrored branch
    /// above already does. Returns true if the state actually changed.
    pub fn apply_scan_state(&mut self, status: Option<&str>, now_ms: i64) -> bool {
        if self.state == SessionState::Ended {
            return false;
        }
        let mapped = match status {
            Some("busy") | Some("shell") => Some(SessionState::Thinking),
            Some("waiting") => Some(SessionState::NeedsInput),
            Some("idle") => Some(SessionState::Idle),
            _ => None,
        };

        if !self.heard || self.state == SessionState::Unknown {
            self.scan_disagreements = 0;
            let Some(mapped) = mapped else { return false };
            if self.state == mapped {
                return false;
            }
            self.set_state(mapped, now_ms);
            return true;
        }

        // ADR-036 tie-break: a heard, already-coloured session normally
        // keeps its hook-set colour (ADR-035), but two consecutive scans
        // that contradict it, with no hook event in between, mean the
        // scan's coarse colour wins.
        let disagrees = matches!(
            (self.state, mapped),
            (SessionState::Thinking, Some(SessionState::Idle))
                | (
                    SessionState::Idle | SessionState::Complete | SessionState::Error,
                    Some(SessionState::Thinking)
                )
        );
        if !disagrees {
            self.scan_disagreements = 0;
            return false;
        }
        self.scan_disagreements += 1;
        if self.scan_disagreements < SCAN_TIEBREAK_SCANS {
            return false;
        }
        self.scan_disagreements = 0;
        match mapped {
            Some(SessionState::Idle) => {
                // Mirrors the is_interrupt branch of PostToolUseFailure
                // (apply_hook): a session the CLI calls idle has nothing
                // in flight.
                self.open_ops.clear();
                self.children = 0;
                self.child_ids.clear();
                self.pending_complete = false;
                self.set_state(SessionState::Idle, now_ms);
            }
            Some(SessionState::Thinking) => {
                // Mirrors UserPromptSubmit (apply_hook): a new turn whose
                // own event was lost.
                self.pending_complete = false;
                self.error = None;
                self.set_state(SessionState::Thinking, now_ms);
            }
            _ => unreachable!("disagrees implies mapped is Idle or Thinking"),
        }
        true
    }

    /// The session's OS process is confirmed gone without a `SessionEnd`
    /// ever arriving (ADR-035, `liveness.rs`): moves it to `Ended` the
    /// same way a real `SessionEnd` does. A no-op, returning false, on a
    /// session already `Ended`.
    pub fn process_exited(&mut self, now_ms: i64) -> bool {
        if self.state == SessionState::Ended {
            return false;
        }
        self.end_session(now_ms);
        true
    }

    /// The `T_unknown` deadline (ADR-035). `alive` is whether the
    /// registry holds a live OS process handle for this session right
    /// now. Ended and already-`unknown` sessions never move here.
    ///
    /// Without a live handle, silence past `T_unknown` from the more
    /// recent of the last hook event or the last scan sighting greys the
    /// session, exactly as before the scan carried a status.
    ///
    /// With a live handle, the process itself stands in for every colour
    /// except `Thinking`: idle, complete, error and needs-input never
    /// grey while it is alive, since none of them claims work is
    /// happening that only a hook could report finishing. A `Thinking`
    /// session is the one case the process cannot vouch for on its own
    /// (a live process says nothing about whether it is still doing the
    /// thing hooks last said it was doing), so it still greys once hooks
    /// have been silent past `T_unknown` and the scan does not say the
    /// turn is still busy or in a shell.
    ///
    /// Never to error either way: a long tool call is normal, and a
    /// wrong red costs more than an honest grey.
    pub fn tick(&mut self, now_ms: i64, alive: bool) -> bool {
        if self.state == SessionState::Ended || self.state == SessionState::Unknown {
            return false;
        }
        if !alive {
            let last_seen = self.last_event_at_ms.max(self.last_seen_ms);
            if now_ms - last_seen > T_UNKNOWN_MS {
                self.set_state(SessionState::Unknown, now_ms);
                return true;
            }
            return false;
        }
        if self.state == SessionState::Thinking
            && now_ms - self.last_event_at_ms > T_UNKNOWN_MS
            && !self.scan_says_in_flight()
        {
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
        assert!(!x.tick(T_UNKNOWN_MS, false));
        assert!(x.tick(T_UNKNOWN_MS + 2, false));
        assert_eq!(x.state, SessionState::Unknown);
        assert!(x.heard, "a timed-out session was heard from; the surface words it apart from never-heard");
        // Any authoritative event leaves unknown.
        ev(&mut x, T_UNKNOWN_MS + 3, json!({"hook_event_name": "UserPromptSubmit"}));
        assert_eq!(x.state, SessionState::Thinking);
    }

    // ---- ADR-035: liveness from the process, state from the scan ------

    #[test]
    fn scan_colours_an_unheard_session_for_each_recognised_status() {
        for (status, want) in [
            ("busy", SessionState::Thinking),
            ("shell", SessionState::Thinking),
            ("waiting", SessionState::NeedsInput),
            ("idle", SessionState::Idle),
        ] {
            let mut x = s();
            assert!(!x.heard);
            assert!(x.apply_scan_state(Some(status), 1), "status {status} must colour an unheard session");
            assert_eq!(x.state, want, "status {status}");
            assert!(!x.heard, "the scan must never set heard");
        }
    }

    #[test]
    fn scan_with_an_unrecognised_status_leaves_the_session_unknown() {
        let mut x = s();
        assert!(!x.apply_scan_state(Some("frobnicating"), 1));
        assert_eq!(x.state, SessionState::Unknown);
        assert!(!x.apply_scan_state(None, 2));
        assert_eq!(x.state, SessionState::Unknown);
    }

    #[test]
    fn a_heard_complete_session_stays_complete_when_the_scan_says_idle() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state, SessionState::Complete);
        assert!(!x.apply_scan_state(Some("idle"), 2), "a hook has already coloured this session");
        assert_eq!(x.state, SessionState::Complete);
    }

    #[test]
    fn a_heard_then_unknown_session_is_recoloured_by_the_scan() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(x.tick(1 + T_UNKNOWN_MS + 1, false));
        assert_eq!(x.state, SessionState::Unknown);
        assert!(x.heard);
        assert!(x.apply_scan_state(Some("busy"), 1 + T_UNKNOWN_MS + 2));
        assert_eq!(x.state, SessionState::Thinking, "unknown is recolourable even once heard");
    }

    #[test]
    fn alive_idle_survives_twenty_minutes_of_silence() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionStart", "source": "startup"}));
        assert_eq!(x.state, SessionState::Idle);
        let twenty_min = 20 * 60 * 1000;
        assert!(!x.tick(1 + twenty_min, true));
        assert_eq!(x.state, SessionState::Idle, "an idle session must not grey while its process is alive");
    }

    #[test]
    fn alive_thinking_with_scan_busy_stays_thinking_after_twenty_minutes() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        x.note_seen(Some("busy"), 2);
        let twenty_min = 20 * 60 * 1000;
        assert!(!x.tick(1 + twenty_min, true));
        assert_eq!(x.state, SessionState::Thinking, "the scan still confirms the turn is in flight");
    }

    #[test]
    fn alive_thinking_with_scan_idle_greys_after_fifteen_minutes() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        x.note_seen(Some("idle"), 2);
        assert!(x.tick(1 + T_UNKNOWN_MS + 1, true), "the two channels disagree, so the colour is no longer trustworthy");
        assert_eq!(x.state, SessionState::Unknown);
    }

    #[test]
    fn alive_needs_input_never_greys() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Notification", "notification_type": "agent_needs_input"}));
        assert_eq!(x.state, SessionState::NeedsInput);
        let one_hour = 60 * 60 * 1000;
        assert!(!x.tick(1 + one_hour, true));
        assert_eq!(x.state, SessionState::NeedsInput);
    }

    #[test]
    fn not_alive_idle_stays_idle_when_the_scan_is_more_recent_than_the_hooks() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionStart", "source": "startup"}));
        let twenty_min = 20 * 60 * 1000;
        let five_min = 5 * 60 * 1000;
        x.note_seen(Some("idle"), 1 + twenty_min - five_min);
        assert!(!x.tick(1 + twenty_min, false), "the scan saw it only five minutes ago");
        assert_eq!(x.state, SessionState::Idle);
    }

    // ---- ADR-036: the scan tie-break ---------------------------------

    #[test]
    fn thinking_survives_a_single_disagreeing_scan() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(!x.apply_scan_state(Some("idle"), 2));
        assert_eq!(x.state, SessionState::Thinking);
        assert_eq!(x.scan_disagreements, 1);
    }

    #[test]
    fn a_second_consecutive_disagreeing_scan_flips_thinking_to_idle() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SubagentStart", "agent_id": "a1"}));
        ev(&mut x, 2, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 3, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state, SessionState::Thinking, "complete is unreachable while the ledger is non-empty");
        assert!(x.pending_complete);
        assert!(!x.apply_scan_state(Some("idle"), 4), "one disagreeing scan is not enough");
        assert_eq!(x.state, SessionState::Thinking);
        assert!(x.apply_scan_state(Some("idle"), 5), "a second consecutive disagreeing scan flips it");
        assert_eq!(x.state, SessionState::Idle);
        assert!(x.open_ops.is_empty(), "a session the CLI calls idle has nothing in flight");
        assert_eq!(x.children, 0);
        assert!(!x.pending_complete);
        assert_eq!(x.scan_disagreements, 0, "the counter resets once it fires");
    }

    #[test]
    fn a_hook_event_between_two_disagreeing_scans_resets_the_count() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(!x.apply_scan_state(Some("idle"), 2));
        assert_eq!(x.scan_disagreements, 1);
        ev(&mut x, 3, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        assert_eq!(x.scan_disagreements, 0, "any hook event resets the count");
        assert!(!x.apply_scan_state(Some("idle"), 4), "this is only the first disagreement again");
        assert_eq!(x.state, SessionState::Thinking);
    }

    #[test]
    fn complete_flips_to_thinking_after_two_busy_scans() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state, SessionState::Complete);
        assert!(!x.apply_scan_state(Some("busy"), 2));
        assert!(x.apply_scan_state(Some("busy"), 3));
        assert_eq!(x.state, SessionState::Thinking, "a lost UserPromptSubmit is recovered from the scan");
    }

    #[test]
    fn error_flips_to_thinking_after_two_busy_scans() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "StopFailure", "error": {"type": "api_error"}}));
        assert_eq!(x.state, SessionState::Error);
        assert!(!x.apply_scan_state(Some("busy"), 2));
        assert!(x.apply_scan_state(Some("busy"), 3));
        assert_eq!(x.state, SessionState::Thinking);
        assert!(x.error.is_none(), "the tie-break mirrors UserPromptSubmit, which clears error");
    }

    #[test]
    fn needs_input_never_flips_to_idle() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "Notification", "notification_type": "agent_needs_input"}));
        assert!(!x.apply_scan_state(Some("idle"), 2));
        assert!(!x.apply_scan_state(Some("idle"), 3));
        assert_eq!(x.state, SessionState::NeedsInput, "needs_input never counts as a disagreement, in any case");
    }

    #[test]
    fn thinking_never_flips_on_waiting() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(!x.apply_scan_state(Some("waiting"), 2));
        assert!(!x.apply_scan_state(Some("waiting"), 3));
        assert_eq!(x.state, SessionState::Thinking, "waiting is the scan's own amber, not a disagreement");
    }

    #[test]
    fn idle_agreeing_with_idle_is_a_no_op_and_never_counts() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionStart", "source": "startup"}));
        assert_eq!(x.state, SessionState::Idle);
        assert!(!x.apply_scan_state(Some("idle"), 2));
        assert!(!x.apply_scan_state(Some("idle"), 3));
        assert_eq!(x.state, SessionState::Idle);
        assert_eq!(x.scan_disagreements, 0, "plain agreement is never a disagreement");
    }

    #[test]
    fn an_unrecognised_scan_between_two_disagreeing_ones_resets_the_count() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(!x.apply_scan_state(Some("idle"), 2));
        assert!(!x.apply_scan_state(Some("frobnicating"), 3), "an unrecognised status resets the count");
        assert!(!x.apply_scan_state(Some("idle"), 4), "only the first disagreement again after the reset");
        assert_eq!(x.state, SessionState::Thinking);
    }

    #[test]
    fn the_immediate_path_still_colours_an_unheard_session_on_the_first_scan() {
        let mut x = s();
        assert!(!x.heard);
        assert!(x.apply_scan_state(Some("idle"), 1), "an unheard session is coloured on the first scan, not the second");
        assert_eq!(x.state, SessionState::Idle);
        assert_eq!(x.scan_disagreements, 0);
    }

    #[test]
    fn process_exited_ends_the_session_and_session_start_revives_it() {
        let mut x = s();
        x.pid = Some(4242);
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert!(x.process_exited(2));
        assert_eq!(x.state, SessionState::Ended);
        assert_eq!(x.pid, None);
        assert!(!x.process_exited(3), "an already-ended session is a no-op");

        assert!(ev(&mut x, 4, json!({"hook_event_name": "SessionStart", "source": "resume"})));
        assert_eq!(x.state, SessionState::Idle, "a resume after process_exited revives the session exactly like after SessionEnd");
    }

    #[test]
    fn a_session_first_seen_by_enumeration_is_unknown_not_idle() {
        let x = Session::new("enumerated".into(), 5);
        assert_eq!(x.state, SessionState::Unknown, "never guess idle");
        assert!(!x.heard, "no hook event has arrived yet");
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
    fn a_subagent_payload_arriving_first_does_not_set_cwd() {
        // SubagentStart carries agent_id and, on this machine, its own
        // cwd rather than the parent session's. If that cwd latched, it
        // would poison Reveal for the whole session (registry.rs is the
        // correction path once enumeration reports the real cwd).
        let mut x = s();
        ev(&mut x, 1, json!({
            "hook_event_name": "SubagentStart",
            "agent_id": "a1",
            "agent_type": "Explore",
            "cwd": "C:\\Users\\o\\dev\\undertow\\subagent-scratch"
        }));
        assert_eq!(x.cwd, None, "a subagent's own cwd must never be taken as the session's");
        assert_eq!(x.label, "", "no cwd means no derived label either");
        // The parent's own event still sets it normally afterward.
        ev(&mut x, 2, json!({"hook_event_name": "UserPromptSubmit", "cwd": "C:\\Users\\o\\dev\\undertow"}));
        assert_eq!(x.cwd.as_deref(), Some("C:\\Users\\o\\dev\\undertow"));
        assert_eq!(x.label, "undertow");
    }

    #[test]
    fn permission_mode_travels_on_any_payload() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit", "permission_mode": "auto"}));
        assert_eq!(x.permission_mode.as_deref(), Some("auto"));
    }

    #[test]
    fn dir_name_handles_windows_posix_trailing_and_edge_cases() {
        assert_eq!(dir_name("C:\\Users\\o\\dev\\undertow"), "undertow");
        assert_eq!(dir_name("/home/o/dev/undertow"), "undertow");
        assert_eq!(dir_name("/home/o/dev/undertow/"), "undertow", "a trailing separator must not leave an empty name");
        assert_eq!(dir_name("bare-name"), "bare-name");
        assert_eq!(dir_name(""), "");
    }

    #[test]
    fn truncate_at_and_past_the_boundary() {
        assert_eq!(truncate("hello", 5), "hello", "exactly at the boundary must not be cut");
        assert_eq!(truncate("hello!", 5), "hello...", "one byte past the boundary is cut and marked");
    }

    #[test]
    fn parse_question_reads_the_first_question_and_its_option_labels() {
        let input = json!({"questions": [{"question": "Which?", "options": [{"label": "A"}, {"label": "B"}]}]});
        let (q, opts) = parse_question(Some(&input));
        assert_eq!(q.as_deref(), Some("Which?"));
        assert_eq!(opts, vec!["A", "B"]);
    }

    #[test]
    fn parse_question_on_missing_or_malformed_input_gives_no_question_and_no_options() {
        assert_eq!(parse_question(None), (None, Vec::new()));
        assert_eq!(parse_question(Some(&json!({"not_questions": []}))), (None, Vec::new()));
        assert_eq!(parse_question(Some(&json!({"questions": []}))), (None, Vec::new()));
        assert_eq!(parse_question(Some(&json!({"questions": [{"no_question_field": true}]}))), (None, Vec::new()));
    }

    #[test]
    fn post_tool_use_closes_only_the_matching_operation() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 2, json!({"hook_event_name": "PreToolUse", "tool_name": "Read", "tool_use_id": "t2"}));
        ev(&mut x, 3, json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        assert_eq!(x.open_ops.len(), 1);
        assert_eq!(x.open_ops[0].id.as_deref(), Some("t2"), "the unrelated operation stays open");
    }

    #[test]
    fn post_tool_use_failure_without_interrupt_closes_only_its_own_op() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        ev(&mut x, 2, json!({"hook_event_name": "PreToolUse", "tool_name": "Read", "tool_use_id": "t2"}));
        let long = "z".repeat(300);
        ev(&mut x, 3, json!({"hook_event_name": "PostToolUseFailure", "tool_name": "Bash", "tool_use_id": "t1", "error": long, "is_interrupt": false}));
        assert_eq!(x.open_ops.len(), 1);
        assert_eq!(x.open_ops[0].id.as_deref(), Some("t2"), "a non-interrupt failure closes only its own bracket");
        assert!(x.error.as_ref().unwrap().message.as_ref().unwrap().ends_with("..."), "the long error text is truncated");
    }

    #[test]
    fn stop_with_live_children_sets_pending_complete_flag() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SubagentStart", "agent_id": "a1"}));
        ev(&mut x, 2, json!({"hook_event_name": "Stop"}));
        assert!(x.pending_complete, "green is deferred while a child is still open");
        ev(&mut x, 3, json!({"hook_event_name": "SubagentStop", "agent_id": "a1"}));
        assert!(!x.pending_complete, "the flag clears once the ledger empties");
        assert_eq!(x.state, SessionState::Complete);
    }

    #[test]
    fn notification_with_an_unknown_type_only_updates_liveness() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        let changed = ev(&mut x, 5, json!({"hook_event_name": "Notification", "notification_type": "mystery"}));
        assert!(!changed);
        assert_eq!(x.state, SessionState::Thinking, "an unrecognised notification type must not move state");
        assert_eq!(x.last_event_at_ms, 5, "liveness still updates");
    }

    #[test]
    fn ended_session_ignores_every_event_except_session_start() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionEnd", "reason": "exit"}));
        assert_eq!(x.state, SessionState::Ended);
        assert!(
            !ev(&mut x, 2, json!({"hook_event_name": "Stop"})),
            "a straggler Stop must report no change"
        );
        assert_eq!(x.state, SessionState::Ended, "a straggler Stop must not revive an ended session");
        assert!(
            !ev(&mut x, 3, json!({"hook_event_name": "PostToolUseFailure", "error": "boom", "is_interrupt": false})),
            "a straggler PostToolUseFailure must report no change"
        );
        assert_eq!(x.state, SessionState::Ended, "a straggler failure must not revive an ended session either");
        assert!(x.error.is_none(), "an ignored straggler must not even record its error detail");
    }

    #[test]
    fn session_end_clears_the_pid() {
        let mut x = s();
        x.pid = Some(4242);
        ev(&mut x, 1, json!({"hook_event_name": "SessionEnd", "reason": "exit"}));
        assert_eq!(x.pid, None, "the OS may reuse an ended session's pid for something else entirely");
    }

    #[test]
    fn session_start_revives_an_ended_session() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "SessionEnd", "reason": "exit"}));
        assert_eq!(x.state, SessionState::Ended);
        assert!(ev(&mut x, 2, json!({"hook_event_name": "SessionStart", "source": "resume"})));
        assert_eq!(x.state, SessionState::Idle, "a resume legitimately revives an ended session");
    }

    #[test]
    fn state_since_ms_moves_only_on_a_state_change() {
        let mut x = s();
        ev(&mut x, 1, json!({"hook_event_name": "UserPromptSubmit"}));
        assert_eq!(x.state_since_ms, 1);
        ev(&mut x, 5, json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_use_id": "t1"}));
        assert_eq!(x.state_since_ms, 1, "staying in thinking must not bump the timestamp");
        ev(&mut x, 9, json!({"hook_event_name": "Stop"}));
        assert_eq!(x.state_since_ms, 9, "an actual state change updates it");
    }
}
