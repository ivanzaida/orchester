//! The runner process as the desktop holds it, and how it ends when the desktop lets go of it:
//!
//! - Windows: the runner runs in a kill-on-close Job Object. Dropping the handle closes the job, which terminates the
//!   runner and every agent it started at once; so does a desktop crash.
//! - macOS: there are no Job Objects. Dropping the handle sends the runner SIGTERM; the runner then ends its agents'
//!   process groups and exits (`orchester-runner`, `shutdown_signal`). A runner still running after [`TERMINATE_GRACE`]
//!   gets SIGKILL, if the desktop is still there to send it. When the desktop crashes, its end of the connection
//!   closes, and the runner ends its agents and exits the same way.

use std::{
  io,
  path::Path,
  process::{ExitStatus, Stdio},
};

use tokio::{process::Child, task::JoinHandle};

use super::{LaunchError, drain_stderr};
use crate::{EXIT_PROJECT_LOCKED, ErrorReports};

/// How long the runner may take to end its agents and exit after SIGTERM: it ends one run after
/// another, each agent's process group with a grace period of its own
/// (`orchester_exec::process_group`).
#[cfg(unix)]
const TERMINATE_GRACE: std::time::Duration = std::time::Duration::from_secs(30);

/// The runner process; see the module documentation for what dropping it does.
pub struct RunnerProcess {
  /// Taken only when the handle is dropped (macOS), to end the runner after it.
  child: Option<Child>,
  stderr: JoinHandle<String>,
  #[cfg(windows)]
  _job: crate::KillOnCloseJob,
}

impl RunnerProcess {
  pub(super) fn spawn(
    runner_exe: &Path,
    project_root: &Path,
    reports: Option<&ErrorReports>,
  ) -> Result<Self, LaunchError> {
    let mut command = tokio::process::Command::new(runner_exe);
    command
      .arg(project_root)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::piped());
    ErrorReports::pass(reports, &mut command);
    platform::prepare(&mut command);
    let mut child = command.spawn().map_err(|source| LaunchError::Spawn {
      exe: runner_exe.to_path_buf(),
      source,
    })?;
    #[cfg(windows)]
    let job = platform::contain(&child)?;
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let stderr = tokio::spawn(drain_stderr(stderr));
    Ok(Self {
      child: Some(child),
      stderr,
      #[cfg(windows)]
      _job: job,
    })
  }

  fn child(&mut self) -> &mut Child {
    self
      .child
      .as_mut()
      .expect("the child is taken only when the handle is dropped")
  }

  /// The operating system process id, while the runner is running.
  pub fn id(&self) -> Option<u32> {
    self.child.as_ref().and_then(Child::id)
  }

  /// Waits for the runner to exit, e.g. after [`crate::Event::Stopped`] and closing the connection.
  pub async fn wait(&mut self) -> io::Result<ExitStatus> {
    self.child().wait().await
  }

  pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
    self.child().try_wait()
  }

  /// Kills a runner that failed to start and waits until it is gone. It never accepted a
  /// connection, so it has started no agents that would outlive it.
  pub(super) async fn end(&mut self) -> io::Result<()> {
    if self.try_wait()?.is_some() {
      return Ok(());
    }
    self.child().kill().await
  }

  pub(super) async fn exited(&mut self, status: ExitStatus) -> LaunchError {
    if status.code() == Some(i32::from(EXIT_PROJECT_LOCKED)) {
      return LaunchError::ProjectLocked;
    }
    let stderr = match (&mut self.stderr).await {
      Ok(stderr) => stderr,
      Err(error) => format!("runner diagnostics unavailable: {error}"),
    };
    LaunchError::RunnerExited {
      status,
      stderr: stderr.trim().to_owned(),
    }
  }
}

#[cfg(windows)]
mod platform {
  use tokio::process::{Child, Command};

  use crate::{KillOnCloseJob, client::LaunchError};

  /// The desktop is a GUI process; without this flag every console child opens a window.
  const CREATE_NO_WINDOW: u32 = 0x0800_0000;

  pub(super) fn prepare(command: &mut Command) {
    command
      .creation_flags(CREATE_NO_WINDOW)
      // Covers the window between spawning and joining the job.
      .kill_on_drop(true);
  }

  pub(super) fn contain(child: &Child) -> Result<KillOnCloseJob, LaunchError> {
    let job = KillOnCloseJob::new().map_err(LaunchError::Job)?;
    let handle = child
      .raw_handle()
      .ok_or_else(|| LaunchError::Job(std::io::Error::other("the runner exited before it could join the job")))?;
    job.assign(handle).map_err(LaunchError::Job)?;
    Ok(job)
  }
}

#[cfg(unix)]
mod platform {
  use rustix::process::{Pid, Signal, kill_process};
  use tokio::process::{Child, Command};

  use super::{RunnerProcess, TERMINATE_GRACE};

  /// Dropping the handle ends the runner through SIGTERM (see [`RunnerProcess`]'s `Drop`),
  /// not with tokio's immediate SIGKILL, which would leave its agents running. The runner gets
  /// the full search path as its `PATH`, for the git it runs, the programs of command tools and
  /// what its agents inherit (`orchester_exec::search_path`).
  pub(super) fn prepare(command: &mut Command) {
    command.kill_on_drop(false);
    if let Some(path) = orchester_exec::search_path::search_path() {
      command.env("PATH", path);
    }
  }

  impl Drop for RunnerProcess {
    fn drop(&mut self) {
      let Some(mut child) = self.child.take() else {
        return;
      };
      match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(error) => tracing::warn!("cannot tell whether the runner still runs: {error}"),
      }
      terminate(&child);
      match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
          runtime.spawn(kill_after_grace(child));
        }
        // No runtime left to wait on: the desktop is exiting, which also closes its connection.
        // Killing the runner now would leave its agents running, so it is left to end them and
        // exit on its own.
        Err(_) => tracing::info!("the runner ends its agents and exits on its own"),
      }
    }
  }

  /// Asks the runner to end its agents and exit.
  fn terminate(child: &Child) {
    let Some(pid) = child.id().and_then(|id| i32::try_from(id).ok()).and_then(Pid::from_raw) else {
      return;
    };
    if let Err(error) = kill_process(pid, Signal::TERM) {
      tracing::warn!("cannot ask the runner to stop: {error}");
    }
  }

  async fn kill_after_grace(mut child: Child) {
    if tokio::time::timeout(TERMINATE_GRACE, child.wait()).await.is_ok() {
      return;
    }
    tracing::warn!("the runner did not exit within {TERMINATE_GRACE:?} of SIGTERM; killing it");
    if let Err(error) = child.kill().await {
      tracing::warn!("cannot kill the runner: {error}");
    }
  }
}
