// The reveal worker: a single, dedicated thread that runs Reveal's
// (potentially slow, up to a few seconds -- reveal.rs's `run_code_cli`
// bounds it at 5s) window-raising work off the Tauri webview/event
// thread and off the async runtime's own worker threads.
//
// PR review (blocking): a synchronous reveal used to run straight
// inside a Tauri command on the IPC/event thread, freezing every click,
// drag, and Quit for as long as it took. One serial worker fixes that
// and, as a side effect, fixes ordering too: because submissions are
// drained strictly one at a time in the order they arrived, an older,
// slower reveal can never raise a window after a newer one has already
// completed.
//
// Generic over the reveal function itself (`RevealQueue::spawn` takes
// any `Fn(&RevealRequest) -> String`), and the request/reply types are
// plain owned data, so the whole thing is tested with a fake function
// and no webview, Win32 call, or real reveal timing at all.

use std::sync::mpsc;

use crate::registry::RevealRequest;

struct Job {
    request: RevealRequest,
    reply: mpsc::Sender<String>,
}

/// The worker thread is no longer there to answer: it panicked
/// processing an earlier job, its reply channel was dropped, or this
/// `RevealQueue` outlived it some other way. Carries nothing beyond
/// that fact; every caller already knows which request it submitted
/// and treats this the same as any other Reveal miss.
#[derive(Debug)]
pub struct WorkerGone;

/// Owns the worker thread's job queue. Cloning a `RevealQueue` (wrap it
/// in `Arc`, as `main.rs` does) is cheap: every clone shares the same
/// underlying thread and channel.
pub struct RevealQueue {
    jobs: mpsc::Sender<Job>,
}

impl RevealQueue {
    /// Start the worker thread. `reveal_fn` is the only thing that ever
    /// runs on it; it receives just the owned `RevealRequest`, never a
    /// registry guard or a borrowed `Session` (PR review: no registry
    /// guard or borrowed Session crosses into the worker or an await --
    /// the type signature here is what makes that true, not just a
    /// convention callers have to remember).
    pub fn spawn<F>(reveal_fn: F) -> RevealQueue
    where
        F: Fn(&RevealRequest) -> String + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("deckhand-reveal".into())
            .spawn(move || {
                for job in rx {
                    let text = reveal_fn(&job.request);
                    // The receiving end (the command's spawn_blocking
                    // closure) may itself have given up -- it never
                    // does today, but nothing here should panic if a
                    // future caller adds a timeout on its side. Either
                    // way there is nothing useful to do with a failed
                    // send: the next job is still owed the same
                    // ordering guarantee.
                    let _ = job.reply.send(text);
                }
                // Reached only once every RevealQueue (and every clone
                // of its Sender) has been dropped, or a panic above
                // unwound out of the loop -- either way `rx` (and with
                // it every future job's Sender-side attempt) closes
                // here, so `submit` starts reporting the worker as
                // gone.
            })
            .expect("spawn reveal worker thread");
        RevealQueue { jobs: tx }
    }

    /// Hand one request to the worker and return a receiver for its
    /// reply. `Err(WorkerGone)` means the worker thread is no longer
    /// there to receive it; the caller treats that exactly like a reply
    /// that never arrived.
    pub fn submit(&self, request: RevealRequest) -> Result<mpsc::Receiver<String>, WorkerGone> {
        let (reply_tx, reply_rx) = mpsc::channel::<String>();
        self.jobs.send(Job { request, reply: reply_tx }).map_err(|_| WorkerGone)?;
        Ok(reply_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use serde_json::json;
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::Duration;

    fn req(id: &str) -> RevealRequest {
        RevealRequest { session_id: id.to_string(), label: String::new(), cwd: None, dir: None, pid: None }
    }

    #[test]
    fn two_requests_complete_in_submission_order_regardless_of_relative_delay() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let order_w = order.clone();
        let queue = RevealQueue::spawn(move |r: &RevealRequest| {
            if r.session_id == "slow" {
                std::thread::sleep(Duration::from_millis(60));
            }
            order_w.lock().unwrap().push(r.session_id.clone());
            r.session_id.clone()
        });

        // The slower request is submitted first; a naive "whichever
        // finishes first wins" scheme would let "fast" complete before
        // it. The serial worker must not allow that: "fast" cannot even
        // start until "slow" is done.
        let rx_slow = queue.submit(req("slow")).expect("worker alive");
        let rx_fast = queue.submit(req("fast")).expect("worker alive");

        assert_eq!(rx_slow.recv().unwrap(), "slow");
        assert_eq!(rx_fast.recv().unwrap(), "fast");
        assert_eq!(
            *order.lock().unwrap(),
            vec!["slow".to_string(), "fast".to_string()],
            "an older, slower request must never be overtaken by a newer, faster one"
        );
    }

    #[test]
    fn the_worker_never_holds_a_registry_lock_while_a_reveal_is_blocked() {
        // A separate registry, untouched by the queue: reveal_fn below
        // only ever receives a RevealRequest, so it has no way to reach
        // this mutex even if it wanted to. Locking and mutating it while
        // a reveal is deliberately stuck proves that in practice, not
        // just by reading the type signature.
        let reg = Arc::new(Mutex::new(Registry::default()));

        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let entered_w = entered.clone();
        let release_w = release.clone();
        let queue = RevealQueue::spawn(move |r: &RevealRequest| {
            entered_w.wait();
            release_w.wait();
            format!("done {}", r.session_id)
        });

        let rx = queue.submit(req("s1")).expect("worker alive");
        entered.wait(); // the worker is now blocked inside reveal_fn

        {
            let mut r = reg.lock().unwrap();
            let changed = r.apply_hook(
                &json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": "unrelated"}),
                1,
            );
            assert!(changed, "an ordinary registry mutation must complete immediately, not wait on the stuck reveal");
        }

        release.wait();
        assert_eq!(rx.recv().unwrap(), "done s1");
    }

    #[test]
    fn a_panicking_reveal_fn_drops_the_reply_without_answering() {
        let queue = RevealQueue::spawn(|r: &RevealRequest| {
            if r.session_id == "boom" {
                panic!("simulated reveal failure");
            }
            "ok".to_string()
        });
        let rx = queue.submit(req("boom")).expect("the send itself still succeeds");
        assert!(
            rx.recv().is_err(),
            "a panicking reveal_fn must drop the reply sender rather than fabricate a result"
        );
    }

    #[test]
    fn a_dead_worker_is_eventually_reported_by_submit_itself() {
        let queue = RevealQueue::spawn(|_r: &RevealRequest| panic!("always dies"));
        let _ = queue.submit(req("first")).unwrap().recv(); // kills the worker thread

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if queue.submit(req("probe")).is_err() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "submit must eventually see the worker thread is gone once it has panicked"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_request_already_built_is_unaffected_by_a_later_removal() {
        // The registry half of this guarantee is pinned in registry.rs
        // (a_request_built_by_begin_activation_is_unaffected_by_a_later_removal);
        // this is the queue half: once a RevealRequest exists, the
        // worker that eventually processes it sees exactly what was in
        // it at submit time, no matter what the registry does meanwhile.
        let mut r = Registry::default();
        for id in ["a", "b"] {
            r.apply_hook(&json!({"hook_event_name": "SessionStart", "source": "startup", "session_id": id}), 1);
        }
        let crate::registry::Activation::Go(request) = r.begin_activation("b", 2) else {
            panic!("b is bound");
        };
        r.apply_hook(&json!({"hook_event_name": "SessionEnd", "reason": "exit", "session_id": "a"}), 3);

        let received: Arc<Mutex<Option<RevealRequest>>> = Arc::new(Mutex::new(None));
        let received_w = received.clone();
        let queue = RevealQueue::spawn(move |r: &RevealRequest| {
            *received_w.lock().unwrap() = Some(r.clone());
            "ok".to_string()
        });
        let rx = queue.submit(request).expect("worker alive");
        assert_eq!(rx.recv().unwrap(), "ok");
        assert_eq!(received.lock().unwrap().as_ref().unwrap().session_id, "b");
    }
}
