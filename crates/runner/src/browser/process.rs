//! The browser's process, started and contained as agents are ([`crate::supervisor`]): on Windows it joins a
//! kill-on-close Job Object, on macOS it leads its own process group. Its output is only logged. Dropping it ends the
//! process with everything it started.

use std::time::Duration;

use orchester_core::command::Command;
use tokio::sync::{oneshot, watch};

use super::BrowserError;
use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

pub(super) struct BrowserProcess {
  /// Ends the process when sent or dropped.
  stop: Option<oneshot::Sender<()>>,
  exit: watch::Receiver<Option<ProcessExit>>,
}

impl BrowserProcess {
  pub(super) fn start(command: &Command) -> Result<Self, BrowserError> {
    let process = Supervised::start(command).map_err(|error| BrowserError::Start(error.to_string()))?;
    let (stop, stopped) = oneshot::channel();
    let (exited, exit) = watch::channel(None);
    tokio::spawn(follow(process, stopped, exited));
    Ok(Self { stop: Some(stop), exit })
  }

  /// How the process ended, once it has.
  pub(super) fn exit(&self) -> Option<ProcessExit> {
    self.exit.borrow().clone()
  }

  /// Waits up to `grace` for the process to exit on its own, as it does after `Browser.close`, then ends it with its
  /// tree. How it ended.
  pub(super) async fn end(mut self, grace: Duration) -> ProcessExit {
    if tokio::time::timeout(grace, self.exit.wait_for(Option::is_some))
      .await
      .is_err()
    {
      tracing::warn!("the browser did not exit within {grace:?}; ending it");
      if let Some(stop) = self.stop.take() {
        // The process may have exited this instant; there is nothing left to end then.
        let _ = stop.send(());
      }
    }
    match self.exit.wait_for(Option::is_some).await {
      Ok(exit) => exit.clone().expect("waited for an exit"),
      Err(_) => ProcessExit::WaitFailed {
        reason: "the browser's process was no longer followed".to_owned(),
      },
    }
  }
}

/// Logs the browser's output until it exits, and ends it once `stop` is sent or dropped.
async fn follow(mut process: Supervised, mut stop: oneshot::Receiver<()>, exited: watch::Sender<Option<ProcessExit>>) {
  let mut stopping = false;
  loop {
    tokio::select! {
      event = process.next() => match event {
        Some(ProcessEvent::Output { stream, line }) => tracing::debug!(?stream, "browser: {line}"),
        Some(ProcessEvent::Exited(exit)) => {
          tracing::info!(?exit, "the project's browser exited");
          exited.send_replace(Some(exit));
          return;
        }
        None => {
          exited.send_replace(Some(ProcessExit::WaitFailed {
            reason: "the browser's process ended without reporting how".to_owned(),
          }));
          return;
        }
      },
      _ = &mut stop, if !stopping => {
        stopping = true;
        process.terminate();
      }
    }
  }
}
