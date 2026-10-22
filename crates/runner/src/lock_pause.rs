//! How the runner pauses while one of its database statements waits for a lock another connection holds, for up to
//! a minute (`orchester_store::pause_lock_waits_with`). The runner calls the store from its runtime's async workers
//! ([`crate::WORKER_THREADS`]), which also serve the desktop, the agents' output, the relays and the timers: a wait
//! that slept a worker would hold all of that up, and as many waits at once as there are workers would stop the
//! runner until the lock is let go.

use std::{thread, time::Duration};

use tokio::runtime::{Handle, RuntimeFlavor};

/// Sleeps for `pause` without holding up the runtime's other tasks. On an async worker of the multi-threaded
/// runtime the worker's other tasks move to a new worker first ([`tokio::task::block_in_place`]; on a thread that
/// already handed them over, as after the first pause of a wait, it only sleeps). On any other thread (a blocking
/// one, a test's single-threaded runtime, none) the sleep holds up nothing else.
pub(crate) fn pause_off_async_workers(pause: Duration) {
  match Handle::try_current() {
    Ok(runtime) if runtime.runtime_flavor() == RuntimeFlavor::MultiThread => {
      tokio::task::block_in_place(|| thread::sleep(pause));
    }
    _ => thread::sleep(pause),
  }
}
