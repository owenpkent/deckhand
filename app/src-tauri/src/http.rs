// The loopback ingest endpoint the shim POSTs to. 127.0.0.1 with a
// per-start token (ADR-007). The token stops accidents, not a determined
// local attacker; in Phase 1 nothing here can act, only observe, and the
// response never carries a decision.

use std::sync::mpsc::Sender;

pub struct HttpServer {
    pub port: u16,
    pub token: String,
}

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
                            && h.value.as_str() == expected
                    });
                let status = if !ok {
                    401
                } else if request.url() != "/hook" {
                    404
                } else {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
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
                };
                let _ = request.respond(tiny_http::Response::empty(status));
            }
        })
        .ok()?;

    Some(HttpServer { port, token })
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    // getrandom over a hand-rolled PRNG: the token guards an endpoint
    // that will later hold permission decisions, so it starts life
    // unguessable even while Phase 1 has nothing worth stealing.
    let _ = getrandom::getrandom(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
