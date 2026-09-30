//! A ripgrep install a person started from Project settings › Code index. It runs on a thread of
//! its own, so it goes on when they leave the page, one at a time, until it ends, is cancelled or
//! runs out of time ([`INSTALL_TIMEOUT`]). On Windows the installer and what it starts are held in a
//! kill-on-close Job Object, so closing Orchester stops them; on macOS they go on after Orchester
//! closes, which the page says.

use std::{
  sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
  },
  time::{Duration, SystemTime},
};

use orchester_exec::ripgrep::install::{InstallError, InstallPlan, Installer};
use parking_lot::Mutex;

/// An install that has not ended after this long is stopped.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
  Running {
    installer: Installer,
    line: String,
    started: SystemTime,
    last_output: Option<String>,
  },
  Failed {
    installer: Installer,
    line: String,
    error: InstallError,
  },
  Installed {
    installer: Installer,
  },
}

static INSTALL: LazyLock<Mutex<Option<Install>>> = LazyLock::new(Mutex::default);
/// Set by *Cancel* while an install runs.
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// The install of this session, if one was started.
pub fn install_state() -> Option<Install> {
  INSTALL.lock().clone()
}

/// Whether closing Orchester stops a running install.
pub const fn ends_with_the_app() -> bool {
  cfg!(windows)
}

/// Starts installing with `plan` unless an install runs already; the caller checks again once it
/// ended.
pub fn start_install(plan: &InstallPlan) {
  let mut install = INSTALL.lock();
  if matches!(*install, Some(Install::Running { .. })) {
    return;
  }
  let (installer, line) = (plan.installer, plan.command_line());
  let installing = match plan.start() {
    Ok(installing) => installing,
    Err(error) => {
      tracing::warn!(%error, "the ripgrep installer could not be started");
      *install = Some(Install::Failed { installer, line, error });
      return;
    }
  };
  CANCELLED.store(false, Ordering::Relaxed);
  let contained = containment::hold(&installing);
  *install = Some(Install::Running {
    installer,
    line: line.clone(),
    started: SystemTime::now(),
    last_output: None,
  });
  std::thread::spawn(move || {
    let finished = installing.wait(
      |output| {
        if let Some(Install::Running { last_output, .. }) = INSTALL.lock().as_mut() {
          *last_output = Some(output.to_owned());
        }
      },
      || CANCELLED.load(Ordering::Relaxed),
      INSTALL_TIMEOUT,
    );
    contained.release();
    *INSTALL.lock() = Some(match finished {
      Ok(()) => Install::Installed { installer },
      Err(error) => {
        tracing::warn!(%error, "ripgrep was not installed");
        Install::Failed { installer, line, error }
      }
    });
  });
}

/// Stops the running install, if one runs.
pub fn cancel_install() {
  CANCELLED.store(true, Ordering::Relaxed);
}

#[cfg(windows)]
mod containment {
  use std::os::windows::io::AsRawHandle;

  use orchester_exec::ripgrep::install::Installing;
  use orchester_ipc::KillOnCloseJob;

  /// A kill-on-close Job Object holding the installer: closing it, or Orchester closing, ends the
  /// installer and everything it started. Without one (it could not be made, which is logged) the
  /// install still runs and a cancel still stops the installer.
  pub(super) struct Contained(Option<KillOnCloseJob>);

  impl Contained {
    /// Ends what the installer left running.
    pub(super) fn release(self) {
      drop(self.0);
    }
  }

  pub(super) fn hold(installing: &Installing) -> Contained {
    let held = KillOnCloseJob::new().and_then(|job| {
      job.assign(installing.process().as_raw_handle())?;
      Ok(job)
    });
    match held {
      Ok(job) => Contained(Some(job)),
      Err(error) => {
        tracing::warn!(%error, "the ripgrep installer is not tied to Orchester's lifetime");
        Contained(None)
      }
    }
  }
}

#[cfg(not(windows))]
mod containment {
  use orchester_exec::ripgrep::install::Installing;

  /// Nothing is held on macOS: the installer leads a process group, which a cancel or the time
  /// limit ends; it goes on when Orchester closes.
  pub(super) struct Contained;

  impl Contained {
    pub(super) const fn release(self) {}
  }

  pub(super) const fn hold(_installing: &Installing) -> Contained {
    Contained
  }
}
