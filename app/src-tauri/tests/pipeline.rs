// End-to-end pipeline test: a raw HTTP POST, through the real ingest
// server and an apply loop shaped like main.rs's, into a Registry, read
// back out through Registry::snapshot. This is the six-session colour
// test run headless, without Tauri or a real shim, updated for the
// unbounded auto-binding list: there is no fixed slot count any more,
// and an ended session never appears in the list at all rather than
// occupying a slot nothing can reclaim.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

use deckhand::http;
use deckhand::registry::Registry;
use deckhand::state::{self, SessionState};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// A hand-rolled request, the same shape as http.rs's own test helper:
// no HTTP client dependency earns its keep for a handful of test posts.
fn post(port: u16, token: &str, body: &serde_json::Value) -> u16 {
    let payload = body.to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the ingest server");
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let req = format!(
        "POST /hook HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nX-Deckhand-Token: {token}\r\nContent-Length: {}\r\n\r\n{}",
        payload.len(),
        payload
    );
    stream.write_all(req.as_bytes()).expect("write request");
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).expect("read response");
    let text = String::from_utf8_lossy(&resp);
    text.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0)
}

// One thread owns applying events to the registry, exactly like
// main.rs's own apply thread, so ordering is deterministic even though
// the whole path is asynchronous from the test's point of view.
fn spawn_apply_loop(rx: mpsc::Receiver<serde_json::Value>, reg: Arc<Mutex<Registry>>) {
    std::thread::spawn(move || {
        for payload in rx {
            let mut r = reg.lock().unwrap();
            r.apply_hook(&payload, now_ms());
        }
    });
}

// The pipeline is asynchronous (HTTP thread -> channel -> apply thread),
// so assertions poll with a bounded wait instead of assuming immediacy.
fn wait_until(reg: &Mutex<Registry>, mut predicate: impl FnMut(&Registry) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if predicate(&reg.lock().unwrap()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn six_sessions_paint_six_distinct_colours_end_to_end() {
    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    let server = http::start(tx).expect("start the ingest server");
    let reg = Arc::new(Mutex::new(Registry::default()));
    spawn_apply_loop(rx, reg.clone());

    // One event per session, each mapping to a distinct tile colour.
    // Needs input via Notification/agent_needs_input rather than
    // AskUserQuestion, and error via StopFailure: PostToolUseFailure
    // keeps the turn (and the tile) in thinking by design, so it cannot
    // stand in for the red state.
    let cases: Vec<(&str, &str, serde_json::Value)> = vec![
        ("sess-idle", "C:/dev/idle-proj", json!({"hook_event_name": "SessionStart", "source": "startup"})),
        ("sess-thinking", "C:/dev/thinking-proj", json!({"hook_event_name": "UserPromptSubmit"})),
        (
            "sess-needs-input",
            "C:/dev/needs-input-proj",
            json!({"hook_event_name": "Notification", "notification_type": "agent_needs_input"}),
        ),
        ("sess-complete", "C:/dev/complete-proj", json!({"hook_event_name": "Stop"})),
        ("sess-error", "C:/dev/error-proj", json!({"hook_event_name": "StopFailure", "error": "boom"})),
        ("sess-ended", "C:/dev/ended-proj", json!({"hook_event_name": "SessionEnd", "reason": "exit"})),
    ];

    for (id, cwd, mut payload) in cases {
        let obj = payload.as_object_mut().unwrap();
        obj.insert("session_id".to_string(), json!(id));
        obj.insert("cwd".to_string(), json!(cwd));
        let status = post(server.port, &server.token, &payload);
        assert_eq!(status, 204, "the shim's payload shape for {id} must be accepted");
    }

    // Five sessions land in the list (idle, thinking, needs-input,
    // complete, error); the ended one never does, since apply_hook
    // unbinds on the very same event that ends it and it was never
    // bound before that.
    let ready = wait_until(&reg, |r| r.bindings.len() == 5 && r.sessions.len() >= 6);
    assert!(ready, "the pipeline must bind the five live sessions within the wait budget");

    {
        let r = reg.lock().unwrap();
        let expect = [
            ("sess-idle", "idle-proj", SessionState::Idle),
            ("sess-thinking", "thinking-proj", SessionState::Thinking),
            ("sess-needs-input", "needs-input-proj", SessionState::NeedsInput),
            ("sess-complete", "complete-proj", SessionState::Complete),
            ("sess-error", "error-proj", SessionState::Error),
        ];
        for (i, (id, label, st)) in expect.iter().enumerate() {
            assert_eq!(r.bindings[i], *id, "row {i} must hold the session heard {i}th");
            let session = &r.sessions[*id];
            assert_eq!(session.state, *st, "{id} must show the state its event maps to");
            assert_eq!(&session.label, label, "{id}'s label must come from its cwd");
        }
        assert!(!r.is_bound("sess-ended"), "an ended session must never occupy a row");
        assert_eq!(r.sessions["sess-ended"].state, SessionState::Ended);
    }

    // A seventh session: there is no fixed slot count, so it binds too,
    // landing after the five already in the list.
    let seventh = json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "sess-seventh",
        "cwd": "C:/dev/seventh-proj",
    });
    let status = post(server.port, &server.token, &seventh);
    assert_eq!(status, 204);
    let seen = wait_until(&reg, |r| r.is_bound("sess-seventh"));
    assert!(seen, "a seventh session must still bind: the list is unbounded");
    {
        let r = reg.lock().unwrap();
        assert_eq!(r.bindings.last().map(String::as_str), Some("sess-seventh"));
    }

    // The T_unknown watchdog: every live session goes quiet, but an
    // ended session is never revisited (tick's own early return).
    {
        let mut r = reg.lock().unwrap();
        let future = now_ms() + state::T_UNKNOWN_MS + 1;
        r.tick(future);
        for id in ["sess-idle", "sess-thinking", "sess-needs-input", "sess-complete", "sess-error", "sess-seventh"] {
            assert_eq!(r.sessions[id].state, SessionState::Unknown, "{id} must go quiet after T_unknown");
        }
        assert_eq!(r.sessions["sess-ended"].state, SessionState::Ended, "an ended session is not revisited by the watchdog");
    }
}

#[test]
fn a_wrong_token_is_rejected_and_changes_nothing() {
    let (tx, rx) = mpsc::channel::<serde_json::Value>();
    let server = http::start(tx).expect("start the ingest server");
    let reg = Arc::new(Mutex::new(Registry::default()));
    spawn_apply_loop(rx, reg.clone());

    let body = json!({
        "hook_event_name": "SessionStart",
        "source": "startup",
        "session_id": "intruder",
        "cwd": "C:/dev/intruder",
    });
    let status = post(server.port, "not-the-real-token", &body);
    assert_eq!(status, 401);

    // A rejected request never reaches the channel, so there is nothing
    // to poll for; give the (otherwise idle) apply loop a moment anyway.
    std::thread::sleep(Duration::from_millis(200));
    let r = reg.lock().unwrap();
    assert!(r.sessions.is_empty(), "a request with the wrong token must never reach the registry");
}
