// Black-box tests for the shim binary. Each test spawns the real
// `deckhand-shim` executable against a fake daemon (or none at all) and
// checks the one contract that matters in Phase 1: stdout stays silent
// and the exit code is always 0, no matter what the daemon does. The
// shim's own timeouts (300 ms connect, 700 ms IO) are why the slow
// cases still finish in seconds, not minutes.
//
// std only, per the shim's own dependency budget. Nothing here touches
// this process's environment: every env var goes to the child through
// Command::env / env_remove.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// One temp dir stands in for %LOCALAPPDATA% for the life of a test.
// Removed on drop so a failed assertion still cleans up, instead of
// leaving a pile of dirs behind in CI.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "deckhand-shim-test-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    // Writes the contact file the shim reads. Only the tests that need
    // a daemon present call this; its absence is itself a test case.
    fn write_contact(&self, body: &str) {
        let deckhand = self.0.join("deckhand");
        std::fs::create_dir_all(&deckhand).expect("create deckhand dir");
        std::fs::write(deckhand.join("daemon.json"), body).expect("write contact file");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Spawned child plus a wait thread. The thread, not the test, blocks on
// exit, so a shim that hangs fails the test via recv_timeout instead of
// stalling the whole run.
struct ShimHandle {
    output_rx: mpsc::Receiver<Output>,
}

impl ShimHandle {
    fn wait(self, timeout: Duration) -> Output {
        self.output_rx
            .recv_timeout(timeout)
            .expect("shim did not exit within the bounded wait")
    }
}

// `localappdata: None` removes the var from the child entirely, rather
// than setting it empty, to cover the "not set at all" case.
fn spawn_shim(localappdata: Option<&Path>, stdin_body: &[u8], debug: bool) -> ShimHandle {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_deckhand-shim"));
    match localappdata {
        Some(dir) => {
            cmd.env("LOCALAPPDATA", dir);
        }
        None => {
            cmd.env_remove("LOCALAPPDATA");
        }
    }
    if debug {
        cmd.env("DECKHAND_SHIM_DEBUG", "1");
    } else {
        cmd.env_remove("DECKHAND_SHIM_DEBUG");
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn shim");
    // Write and close stdin before handing the child to the wait
    // thread: the shim reads stdin to EOF before it ever touches the
    // network, so this has to finish first regardless.
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin_body)
        .expect("write stdin to shim");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out = child.wait_with_output().expect("wait on shim");
        let _ = tx.send(out);
    });
    ShimHandle { output_rx: rx }
}

// What a fake daemon saw from one request.
struct Received {
    request_line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// Reads a request off `stream` up through a Content-Length-complete
// body. A read timeout guards this against a shim that connects but
// never finishes sending.
fn read_request(stream: &mut TcpStream) -> Received {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).expect("read headers from shim");
        assert!(n > 0, "shim closed the connection before sending headers");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim().to_string();
        let v = v.trim().to_string();
        if k.eq_ignore_ascii_case("content-length") {
            content_length = v.parse().unwrap_or(0);
        }
        headers.push((k, v));
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).expect("read body from shim");
        assert!(n > 0, "shim closed the connection before sending the full body");
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Received { request_line, headers, body }
}

// Accepts exactly one connection on `listener`, captures the request,
// writes `response` back, and reports what it saw. Runs on its own
// thread so the caller can bound the wait with recv_timeout; if the
// shim never connects this thread just stays parked until the test
// process exits, which is harmless.
fn serve_once(listener: TcpListener, response: Vec<u8>) -> mpsc::Receiver<Received> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let received = read_request(&mut stream);
            let _ = stream.write_all(&response);
            let _ = tx.send(received);
        }
    });
    rx
}

fn header<'a>(received: &'a Received, name: &str) -> Option<&'a str> {
    received
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn happy_path_posts_request_and_exits_clean() {
    let dir = TempDir::new();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake daemon");
    let port = listener.local_addr().expect("listener addr").port();
    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"abc123\"}}"));

    let payload = br#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#.to_vec();
    let server_rx = serve_once(
        listener,
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
    );

    let shim = spawn_shim(Some(dir.path()), &payload, false);
    let received = server_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("daemon never received a request");
    let output = shim.wait(Duration::from_secs(2));

    assert_eq!(received.request_line, "POST /hook HTTP/1.1");
    assert_eq!(header(&received, "X-Deckhand-Token"), Some("abc123"));
    assert_eq!(header(&received, "Content-Type"), Some("application/json"));
    assert_eq!(
        header(&received, "Content-Length").and_then(|v| v.parse::<usize>().ok()),
        Some(payload.len())
    );
    assert_eq!(received.body, payload);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn no_contact_file_exits_clean() {
    let dir = TempDir::new();
    let shim = spawn_shim(Some(dir.path()), b"{}", false);
    let output = shim.wait(Duration::from_secs(2));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn garbage_contact_file_exits_clean() {
    let dir = TempDir::new();
    dir.write_contact("this is not json at all {{{");
    let shim = spawn_shim(Some(dir.path()), b"{}", false);
    let output = shim.wait(Duration::from_secs(2));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn port_with_no_listener_exits_clean() {
    let dir = TempDir::new();
    // Bind to claim a free port, then drop it so nothing answers there.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind to claim a port");
    let port = listener.local_addr().expect("listener addr").port();
    drop(listener);

    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"none\"}}"));
    let shim = spawn_shim(Some(dir.path()), b"{}", false);
    let output = shim.wait(Duration::from_secs(2));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn daemon_accepts_but_never_responds_exits_clean() {
    let dir = TempDir::new();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake daemon");
    let port = listener.local_addr().expect("listener addr").port();
    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"stall\"}}"));

    std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Hold the socket open well past the shim's own IO timeout
            // so the shim's exit, not a reset from this side, is what
            // ends the test.
            std::thread::sleep(Duration::from_secs(2));
            drop(stream);
        }
    });

    let shim = spawn_shim(Some(dir.path()), b"{}", false);
    let output = shim.wait(Duration::from_secs(5));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn large_body_arrives_intact() {
    let dir = TempDir::new();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake daemon");
    let port = listener.local_addr().expect("listener addr").port();
    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"big\"}}"));

    // 256 KiB of non-repeating bytes so a truncation or an offset bug
    // would not slip past a coarser check.
    let payload: Vec<u8> = (0..256 * 1024).map(|i| (i % 251) as u8).collect();
    let server_rx = serve_once(
        listener,
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
    );

    let shim = spawn_shim(Some(dir.path()), &payload, false);
    let received = server_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("daemon never received a request");
    let output = shim.wait(Duration::from_secs(2));

    assert_eq!(received.body.len(), payload.len());
    assert_eq!(received.body, payload);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn bom_prefixed_body_passes_through_unchanged() {
    let dir = TempDir::new();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake daemon");
    let port = listener.local_addr().expect("listener addr").port();
    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"bom\"}}"));

    // The daemon strips a leading BOM; the shim must not, so this
    // asserts the bytes are forwarded exactly as given.
    let mut payload = vec![0xEFu8, 0xBB, 0xBF];
    payload.extend_from_slice(br#"{"hook_event_name":"Notification"}"#);
    let server_rx = serve_once(
        listener,
        b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
    );

    let shim = spawn_shim(Some(dir.path()), &payload, false);
    let received = server_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("daemon never received a request");
    let output = shim.wait(Duration::from_secs(2));

    assert_eq!(received.body, payload);
    assert!(received.body.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn debug_flag_logs_to_stderr_not_stdout() {
    let dir = TempDir::new();
    let shim = spawn_shim(Some(dir.path()), b"{}", true);
    let output = shim.wait(Duration::from_secs(2));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("deckhand-shim:"),
        "expected a deckhand-shim: line on stderr, got: {stderr:?}"
    );
}

#[test]
fn daemon_200_response_leaves_stdout_silent() {
    let dir = TempDir::new();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake daemon");
    let port = listener.local_addr().expect("listener addr").port();
    dir.write_contact(&format!("{{\"port\":{port},\"token\":\"ok\"}}"));

    let payload = br#"{"hook_event_name":"Stop"}"#.to_vec();
    let response_body = br#"{"decision":"noted"}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        response_body.len(),
        String::from_utf8_lossy(response_body)
    )
    .into_bytes();
    let server_rx = serve_once(listener, response);

    let shim = spawn_shim(Some(dir.path()), &payload, false);
    let _received = server_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("daemon never received a request");
    let output = shim.wait(Duration::from_secs(2));

    // Phase 1: stdout is the future decision channel and stays silent
    // no matter what the daemon answers.
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn missing_localappdata_exits_clean() {
    let shim = spawn_shim(None, b"{}", false);
    let output = shim.wait(Duration::from_secs(2));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}
