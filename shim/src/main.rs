// The Deckhand hook shim. Claude Code runs this on every hooked event.
//
// Contract (docs/ARCHITECTURE.md): read hook JSON on stdin, POST it to
// the daemon on loopback, exit. Phase 1 is observation only, so no
// response body is ever written to stdout and the exit code is always 0:
// if the daemon is down, unreachable, or slow, Claude Code must keep
// working normally. A shim that can stall a session is a defect worse
// than any missed event.
//
// The whole thing is std-only on purpose. Hooks fire as short-lived
// subprocesses, potentially on every tool call across six sessions, so
// startup cost is the budget that matters.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const IO_TIMEOUT: Duration = Duration::from_millis(700);

fn run() -> Option<()> {
    // The daemon writes its ephemeral port and token here at startup.
    // Absent file means no daemon: exit silently, cost one stat call.
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        debug("no LOCALAPPDATA");
        return None;
    };
    let path = std::path::Path::new(&base).join("deckhand").join("daemon.json");
    let Ok(meta) = std::fs::read_to_string(&path) else {
        debug(&format!("no contact file at {}", path.display()));
        return None;
    };

    // Two fields, flat, written by us. Parsed by hand to keep serde and
    // its compile-and-startup weight out of the hot path.
    let port_text = extract(&meta, "\"port\":")?;
    let digits: String = port_text
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let Ok(port) = digits.parse::<u16>() else {
        debug(&format!("bad port in contact file: {digits:?}"));
        return None;
    };
    let token = extract(&meta, "\"token\":\"")?;
    let token = &token[..token.find('"')?];

    let mut body = Vec::with_capacity(4096);
    std::io::stdin().read_to_end(&mut body).ok()?;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT);
    let Ok(mut stream) = stream else {
        debug(&format!("connect to 127.0.0.1:{port} failed"));
        return None;
    };
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;

    let head = format!(
        "POST /hook HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         X-Deckhand-Token: {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(&body).ok()?;
    stream.flush().ok()?;

    // Wait for (and discard) the status line so the daemon has actually
    // received the event before this process exits, bounded by the read
    // timeout above.
    let mut ack = [0u8; 32];
    let n = stream.read(&mut ack).unwrap_or(0);
    debug(&format!(
        "port={port} sent={} ack={:?}",
        body.len(),
        String::from_utf8_lossy(&ack[..n])
    ));
    Some(())
}

// Diagnostics on stderr, only when DECKHAND_SHIM_DEBUG is set. stdout
// stays silent unconditionally: it is the future decision channel.
fn debug(msg: &str) {
    if std::env::var_os("DECKHAND_SHIM_DEBUG").is_some() {
        eprintln!("deckhand-shim: {msg}");
    }
}

fn extract<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let at = text.find(key)? + key.len();
    Some(&text[at..])
}

fn main() {
    let _ = run();
    // Always 0, always silent. See the header comment.
}
