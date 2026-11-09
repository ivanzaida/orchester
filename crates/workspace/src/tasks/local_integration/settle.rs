//! A merge asked for the moment a run ends meets Git programs that are still closing. The bounded wait here looks at
//! them again before a refusal for programs not proved stopped is made; one that is really running outlasts it.
use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_core::integration::local::LocalError;
use orchester_exec::local_git::LocalGitError;

use super::IntegrationError;

/// How long Git programs may take to be proved stopped before the merge refuses.
const SETTLE: Duration = Duration::from_secs(3);
/// How often the wait looks again.
const POLL: Duration = Duration::from_millis(100);

/// The result of `attempt`, made again for up to [`SETTLE`] while it says Git programs are not proved stopped. Any
/// other result, and that one once the wait ran out, is returned as it is: a program still alive is never taken for
/// stopped.
pub(super) fn settled<T>(attempt: impl FnMut() -> Result<T, IntegrationError>) -> Result<T, IntegrationError> {
  wait_while(
    attempt,
    |result| matches!(result, Err(error) if not_proved_stopped(error)),
  )
}

/// What `read` gives, read again for up to [`SETTLE`] while `waiting` says of it that the program it shows is closing.
pub(super) fn wait_while<T>(mut read: impl FnMut() -> T, waiting: impl Fn(&T) -> bool) -> T {
  let deadline = Instant::now() + SETTLE;
  loop {
    let read = read();
    if !waiting(&read) || Instant::now() >= deadline {
      return read;
    }
    thread::sleep(POLL);
  }
}

fn not_proved_stopped(error: &IntegrationError) -> bool {
  matches!(
    error,
    IntegrationError::State(LocalError::ProcessUncertain)
      | IntegrationError::Git(LocalGitError::State(LocalError::ProcessUncertain))
  )
}
