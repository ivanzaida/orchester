//! Process groups on macOS, the counterpart of the Job Objects that hold a program's process tree
//! on Windows. Every program [`crate::prepare`] builds leads a process group of its own, so the
//! program and everything it started can be signalled as one: SIGTERM first, SIGKILL for what is
//! still running after a grace period.
//!
//! Unlike a Job Object, a group can be left on purpose: a process that calls `setsid` or
//! `setpgid` (a daemon) is not ended with it.

use std::{
  io,
  os::unix::process::CommandExt,
  process::Command,
  thread,
  time::{Duration, Instant},
};

use rustix::{
  io::Errno,
  process::{Pid, Signal, kill_process_group, test_kill_process_group},
};

/// How long the processes of a group may take to exit after SIGTERM before they get SIGKILL.
pub const TERMINATE_GRACE: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Makes the program started by `process` the leader of a new process group. The group is set
/// before the program runs, so nothing it starts can escape it by timing.
pub(crate) fn lead_own_group(process: &mut Command) {
  process.process_group(0);
}

/// The process group a program started by [`crate::prepare`] leads; its id is the program's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessGroup {
  leader: Pid,
}

/// How [`ProcessGroup::end`] ended a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupEnd {
  /// No process was left, or all left after SIGTERM within the grace period.
  Exited,
  /// Some process was still running after the grace period and got SIGKILL.
  Killed,
}

impl ProcessGroup {
  /// The group led by the child with process id `child_id`. `None` for an id no child can have.
  pub fn led_by(child_id: u32) -> Option<Self> {
    let leader = Pid::from_raw(i32::try_from(child_id).ok()?)?;
    Some(Self { leader })
  }

  /// Sends SIGTERM to every process of the group, asking them to exit.
  pub fn terminate(self) -> io::Result<()> {
    self.signal(Signal::TERM)
  }

  /// Sends SIGKILL to every process of the group.
  pub fn kill(self) -> io::Result<()> {
    self.signal(Signal::KILL)
  }

  /// Whether no process of the group is left. A leader that has exited but was not reaped yet
  /// may still count, so reap it before waiting for the group to empty.
  pub fn is_gone(self) -> io::Result<bool> {
    match test_kill_process_group(self.leader) {
      Ok(()) => Ok(false),
      Err(Errno::SRCH) => Ok(true),
      // A process of the group belongs to another user (a setuid program): it is still there.
      Err(Errno::PERM) => Ok(false),
      Err(error) => Err(error.into()),
    }
  }

  /// Ends every process still in the group: SIGTERM, up to `grace` for them to exit, then
  /// SIGKILL. Blocks for up to `grace`. Call it once the leader has been reaped.
  pub fn end(self, grace: Duration) -> io::Result<GroupEnd> {
    if self.is_gone()? {
      return Ok(GroupEnd::Exited);
    }
    self.terminate()?;
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
      if self.is_gone()? {
        return Ok(GroupEnd::Exited);
      }
      thread::sleep(POLL_INTERVAL);
    }
    self.kill()?;
    Ok(GroupEnd::Killed)
  }

  /// A group whose processes have all exited has nothing left to signal.
  fn signal(self, signal: Signal) -> io::Result<()> {
    match kill_process_group(self.leader, signal) {
      Ok(()) | Err(Errno::SRCH) => Ok(()),
      Err(error) => Err(error.into()),
    }
  }
}
