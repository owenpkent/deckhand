// Library surface for the daemon: the bin (main.rs) and the integration
// tests both build on this so test code never has to shell out to a
// second copy of the state machine.

pub mod enumerate;
pub mod host;
pub mod http;
pub mod persist;
pub mod registry;
pub mod reveal;
pub mod reveal_queue;
pub mod state;
pub mod window;
