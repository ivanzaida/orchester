//! The tree as the program's own process group (macOS).

use std::{
  io,
  process::Child,
  thread,
  time::{Duration, Instant},
};

use super::TERMINATE_GRACE;
use crate::process_group::ProcessGroup;

const POLL: Duration = Duration::from_millis(20);

/// The group the program leads.
pub(super) struct Hold {
  group: ProcessGroup,
}

impl Hold {
  pub(super) fn of(child: &Child) -> io::Result<Self> {
    let group =
      ProcessGroup::led_by(child.id()).ok_or_else(|| io::Error::other(format!("invalid process id {}", child.id())))?;
    Ok(Self { group })
  }

  /// Asks the group to exit, reaps `child` (killing the group when it does not exit within the grace period), then
  /// ends what of the group is left.
  pub(super) fn end(&mut self, child: &mut Child) -> io::Result<()> {
    self.group.terminate()?;
    let deadline = Instant::now() + TERMINATE_GRACE;
    while child.try_wait()?.is_none() {
      if Instant::now() >= deadline {
        self.group.kill()?;
        child.wait()?;
        break;
      }
      thread::sleep(POLL);
    }
    self.group.end(TERMINATE_GRACE).map(drop)
  }
}
