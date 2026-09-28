//! Being asked to stop by the operating system rather than by the desktop.
//!
//! - macOS: the desktop sends SIGTERM when it lets go of the runner (it has no Job Object to close), and a terminal
//!   sends SIGINT to a development build on Ctrl+C. The runner then ends its agents' process groups and exits, like
//!   "quit now".
//! - Windows: the desktop ends the runner and its agents by closing their Job Object; no signal is ever delivered, so
//!   [`ShutdownSignal::received`] never completes.

use std::io;

#[cfg(unix)]
pub struct ShutdownSignal {
  terminate: tokio::signal::unix::Signal,
  interrupt: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl ShutdownSignal {
  /// Takes over SIGTERM and SIGINT from their default action (ending the process at once).
  /// Must be called within a Tokio runtime.
  pub fn install() -> io::Result<Self> {
    use tokio::signal::unix::{SignalKind, signal};
    Ok(Self {
      terminate: signal(SignalKind::terminate())?,
      interrupt: signal(SignalKind::interrupt())?,
    })
  }

  /// Completes when SIGTERM or SIGINT arrives.
  pub async fn received(&mut self) {
    tokio::select! {
      _ = self.terminate.recv() => tracing::info!("SIGTERM received"),
      _ = self.interrupt.recv() => tracing::info!("SIGINT received"),
    }
  }
}

#[cfg(windows)]
pub struct ShutdownSignal;

#[cfg(windows)]
impl ShutdownSignal {
  pub fn install() -> io::Result<Self> {
    Ok(Self)
  }

  /// Never completes: see the module documentation.
  pub async fn received(&mut self) {
    std::future::pending::<()>().await;
  }
}
