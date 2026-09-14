// The loopback ingest endpoint the shim POSTs to. 127.0.0.1 with a
// per-start token (ADR-007). The token stops accidents, not a determined
// local attacker; in Phase 1 nothing here can act, only observe, and the
// response never carries a decision.

use std::io::Read;
use std::sync::mpsc::Sender;

pub struct HttpServer {
    pub port: u16,
    pub token: String,
}

/// Ceiling on one ingest request body. Hook payloads are prompts and
/// tool inputs, never anything close to this; the limit exists only to
/// stop a malformed or hostile POST from growing memory without bound.
const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;

/// Start the ingest server on an ephemeral loopback port. Each accepted
/// hook payload is sent up the channel; the daemon thread owns all state.
pub fn start(events: Sender<serde_json::Value>) -> Option<HttpServer> {
    let server = tiny_http::Server::http("127.0.0.1:0").ok()?;
    let port = server.server_addr().to_ip()?.port();
    let token = random_token();
    let expected = token.clone();

    std::thread::Builder::new()
        .name("deckhand-http".into())
        .spawn(move || {
            for mut request in server.incoming_requests() {
                let ok = request
                    .headers()
                    .iter()
                    .any(|h| {
                        h.field.as_str().as_str().eq_ignore_ascii_case("x-deckhand-token")
                            && tokens_equal(h.value.as_str(), &expected)
                    });
                let status = if !ok {
                    401
                } else if request.url() != "/hook" {
                    404
                } else if request
                    .headers()
                    .iter()
                    .any(|h| h.field.equiv("Transfer-Encoding"))
                {
                    // Refuse chunked bodies unread. tiny_http's chunked
                    // decoder buffers each chunk-size line without a
                    // bound, below the `take` cap further down, so a
                    // body that is never read is the only safe one. The
                    // shim always sends Content-Length.
                    411
                } else if request.body_length().map_or(false, |n| n as u64 > MAX_BODY_BYTES) {
                    // Declared over the cap: refuse before reading.
                    413
                } else {
                    let mut body = String::new();
                    // Read at most one byte past the limit: that extra
                    // byte is what tells an exactly-at-limit body apart
                    // from an over-limit one without ever buffering more
                    // than MAX_BODY_BYTES + 1.
                    let _ = request
                        .as_reader()
                        .take(MAX_BODY_BYTES + 1)
                        .read_to_string(&mut body);
                    if body.len() as u64 > MAX_BODY_BYTES {
                        // Over the cap: drop the event outright rather
                        // than parse and apply a truncated body.
                        413
                    } else {
                        // Tolerate a UTF-8 BOM: PowerShell test harnesses
                        // prepend one when piping into the shim, and
                        // serde_json rejects it.
                        let body = body.trim_start_matches('\u{feff}');
                        match serde_json::from_str::<serde_json::Value>(body) {
                            Ok(v) => {
                                let _ = events.send(v);
                                204
                            }
                            Err(_) => 400,
                        }
                    }
                };
                let _ = request.respond(tiny_http::Response::empty(status));
            }
        })
        .ok()?;

    Some(HttpServer { port, token })
}

/// Constant-time equality for the ingest token. A plain `==` returns as
/// soon as it finds a mismatched byte, so how long the compare takes
/// leaks how many leading bytes of a guessed token were right; a local
/// attacker who can measure that timing could recover the token one
/// byte at a time. This instead always walks every byte of the shorter
/// operand and folds the differences together, so the time taken
/// depends only on length, never on content. The length check up front
/// is not itself a timing leak worth closing: `expected` is always the
/// same fixed-length hex token (see `random_token`), so its length was
/// never a secret.
fn tokens_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    // getrandom over a hand-rolled PRNG: the token guards an endpoint
    // that will later hold permission decisions, so it starts life
    // unguessable even while Phase 1 has nothing worth stealing.
    let _ = getrandom::getrandom(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::Duration;

    // A hand-rolled request over TcpStream rather than a pulled-in HTTP
    // client: this is the only place the daemon needs one, and it is a
    // handful of lines. Connection: close means the server ends this
    // request's connection once it has replied, so read_to_end returns.
    fn post(port: u16, path: &str, token: Option<&str>, header_name: &str, body: &[u8]) -> u16 {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the ingest server");
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let mut req = format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        if let Some(t) = token {
            req.push_str(&format!("{header_name}: {t}\r\n"));
        }
        req.push_str("\r\n");
        stream.write_all(req.as_bytes()).expect("write request headers");
        stream.write_all(body).expect("write request body");

        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).expect("read response");
        let text = String::from_utf8_lossy(&resp);
        // "HTTP/1.1 204 No Content" -> the second whitespace-separated token.
        text.lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0)
    }

    #[test]
    fn correct_token_hook_valid_json_gives_204_and_the_value_arrives() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", br#"{"a":1}"#);
        assert_eq!(status, 204);
        let v = rx.recv_timeout(Duration::from_millis(500)).expect("the value arrives on the channel");
        assert_eq!(v, serde_json::json!({"a": 1}));
    }

    #[test]
    fn missing_token_gives_401_and_nothing_arrives() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/hook", None, "X-Deckhand-Token", br#"{"a":1}"#);
        assert_eq!(status, 401);
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "a rejected request must never reach the channel");
    }

    #[test]
    fn wrong_token_gives_401() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/hook", Some("not-the-token"), "X-Deckhand-Token", br#"{"a":1}"#);
        assert_eq!(status, 401);
    }

    #[test]
    fn correct_token_wrong_path_gives_404() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/other", Some(&server.token), "X-Deckhand-Token", br#"{"a":1}"#);
        assert_eq!(status, 404);
    }

    #[test]
    fn correct_token_invalid_json_gives_400() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", b"not json");
        assert_eq!(status, 400);
    }

    #[test]
    fn a_leading_utf8_bom_is_accepted() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let mut body = vec![0xEF, 0xBB, 0xBF];
        body.extend_from_slice(br#"{"a":2}"#);
        let status = post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", &body);
        assert_eq!(status, 204);
        let v = rx.recv_timeout(Duration::from_millis(500)).expect("the BOM must be tolerated, not rejected");
        assert_eq!(v, serde_json::json!({"a": 2}));
    }

    #[test]
    fn the_header_name_is_matched_case_insensitively() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let status = post(server.port, "/hook", Some(&server.token), "X-DECKHAND-TOKEN", br#"{"a":3}"#);
        assert_eq!(status, 204);
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_ok());
    }

    #[test]
    fn tokens_equal_accepts_the_same_string() {
        assert!(tokens_equal("abc123", "abc123"));
    }

    #[test]
    fn tokens_equal_rejects_a_same_length_mismatch() {
        assert!(!tokens_equal("abc123", "abc124"));
    }

    #[test]
    fn tokens_equal_rejects_different_lengths() {
        assert!(!tokens_equal("abc123", "abc1234"));
        assert!(!tokens_equal("abc1234", "abc123"));
    }

    #[test]
    fn tokens_equal_rejects_empty_against_nonempty() {
        assert!(!tokens_equal("", "abc123"));
        assert!(tokens_equal("", ""));
    }

    #[test]
    fn a_body_over_the_cap_gives_413_and_nothing_arrives() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        // One byte past the cap: the smallest body that must be rejected.
        let oversized = vec![b'0'; (MAX_BODY_BYTES + 1) as usize];
        let status = post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", &oversized);
        assert_eq!(status, 413);
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "an oversized body must never reach the channel, even in part"
        );
    }

    #[test]
    fn a_chunked_body_gives_411_unread_and_nothing_arrives() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect to the ingest server");
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let head = format!(
            "POST /hook HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Deckhand-Token: {}\r\nTransfer-Encoding: chunked\r\n\r\n",
            server.token
        );
        stream.write_all(head.as_bytes()).expect("write request headers");
        // An oversized chunk-size line: the framing the decoder would
        // have buffered without bound. The server replies without
        // reading it, so a failed write here is fine.
        let _ = stream.write_all(&vec![b'0'; 1024 * 1024]);
        let mut resp = Vec::new();
        let _ = stream.read_to_end(&mut resp);
        let text = String::from_utf8_lossy(&resp);
        assert!(text.starts_with("HTTP/1.1 411"), "got: {}", text.lines().next().unwrap_or(""));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_declared_length_over_the_cap_gives_413_unread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect to the ingest server");
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        let head = format!(
            "POST /hook HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Deckhand-Token: {}\r\nContent-Length: {}\r\n\r\n{{}}",
            server.token,
            MAX_BODY_BYTES + 1
        );
        stream.write_all(head.as_bytes()).expect("write request");
        let mut resp = Vec::new();
        let _ = stream.read_to_end(&mut resp);
        let text = String::from_utf8_lossy(&resp);
        assert!(text.starts_with("HTTP/1.1 413"), "got: {}", text.lines().next().unwrap_or(""));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_body_exactly_at_the_cap_is_still_accepted() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        // Pad valid JSON out to exactly MAX_BODY_BYTES with leading
        // whitespace, which serde_json ignores; pins the cap check at
        // its boundary rather than merely somewhere below it.
        let json = br#"{"a":1}"#;
        let mut body = vec![b' '; MAX_BODY_BYTES as usize - json.len()];
        body.extend_from_slice(json);
        assert_eq!(body.len() as u64, MAX_BODY_BYTES);
        let status = post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", &body);
        assert_eq!(status, 204);
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_ok());
    }

    #[test]
    fn two_sequential_posts_arrive_in_order() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = start(tx).expect("start the ingest server");
        post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", br#"{"n":1}"#);
        post(server.port, "/hook", Some(&server.token), "X-Deckhand-Token", br#"{"n":2}"#);
        let first = rx.recv_timeout(Duration::from_millis(500)).unwrap();
        let second = rx.recv_timeout(Duration::from_millis(500)).unwrap();
        assert_eq!(first, serde_json::json!({"n": 1}));
        assert_eq!(second, serde_json::json!({"n": 2}));
    }
}
