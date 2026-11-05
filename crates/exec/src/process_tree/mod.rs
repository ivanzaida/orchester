//! A long-running program held with everything it starts, for programs Orchester starts and stops itself, such as a
//! run's services: stopping it, or dropping it, ends the whole tree and reaps the program.
//!
//! - Windows ([`windows`]): a kill-on-close Job Object the program joins right after it started; closing it, or the
//!   runner ending, ends every process in it. A child the program starts in that first instant, before it joined, is
//!   not held (the QA script's tree has the same window).
//! - macOS ([`unix`]): the program leads a process group of its own, set before it runs ([`crate::prepare`]); stopping
//!   sends the group SIGTERM, then SIGKILL to what is left after [`TERMINATE_GRACE`]. A runner that is killed leaves
//!   the group running, and a process that leaves the group on purpose (`setsid`) is not stopped with it (`AGENTS.md`,
//!   known gaps).

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::{
  io,
  process::{Child, ChildStderr, ChildStdout, ExitStatus},
  time::Duration,
};

use orchester_core::command::{Command, CommandError};
#[cfg(unix)]
use unix as platform;

#[cfg(windows)]
use self::windows as platform;

/// How long the processes of a tree may take to exit once asked before they are ended.
pub const TERMINATE_GRACE: Duration = Duration::from_secs(3);

/// A started program and everything it starts.
pub struct ProcessTree {
  child: Child,
  hold: platform::Hold,
  ended: bool,
}

impl ProcessTree {
  /// Starts `command` as [`crate::prepare`] builds it, its standard output and error piped, and holds its tree.
  pub fn start(command: &Command) -> Result<Self, CommandError> {
    let mut child = crate::spawn(command)?;
    match platform::Hold::of(&child) {
      Ok(hold) => Ok(Self {
        child,
        hold,
        ended: false,
      }),
      Err(error) => {
        // The program runs unheld: it is ended at once, and the start fails with why.
        if let Err(kill) = child.kill().and_then(|()| child.wait().map(drop)) {
          tracing::warn!(%kill, "a program whose tree could not be held was not ended");
        }
        Err(CommandError::NotStarted(format!(
          "its process tree cannot be held: {error}"
        )))
      }
    }
  }

  pub fn pid(&self) -> u32 {
    self.child.id()
  }

  /// The program's standard output and error, each once.
  pub fn take_output(&mut self) -> (Option<ChildStdout>, Option<ChildStderr>) {
    (self.child.stdout.take(), self.child.stderr.take())
  }

  /// How the program itself exited; `None` while it runs. Processes it started may still run.
  pub fn exited(&mut self) -> io::Result<Option<ExitStatus>> {
    self.child.try_wait()
  }

  /// Ends the program and everything it started, and reaps it. Blocks for up to about twice [`TERMINATE_GRACE`].
  pub fn stop(mut self) -> io::Result<()> {
    self.end()
  }

  fn end(&mut self) -> io::Result<()> {
    if self.ended {
      return Ok(());
    }
    self.ended = true;
    self.hold.end(&mut self.child)
  }
}

impl Drop for ProcessTree {
  fn drop(&mut self) {
    if let Err(error) = self.end() {
      tracing::warn!(pid = self.child.id(), %error, "a program's process tree was not ended");
    }
  }
}
