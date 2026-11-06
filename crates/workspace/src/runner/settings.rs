//! The application settings every project's runner gets: whether it sends error reports
//! ([`Runners::set_error_reports`]) and this computer's Settings › Browser ([`Runners::set_browser`],
//! `docs/architecture/remote-runners.md`), which the runner never reads from the application database itself. A runner
//! gets both when it starts or attaches, and again whenever they change.

use orchester_ipc::{Command, ErrorReports, Reply, browser::ComputerBrowser};

use super::{RequestError, Runners};
use crate::workspace::Workspace;

impl Runners {
  /// The runners send their panics and ERROR lines as error reports with `reports`, or none with `None`
  /// (`orchester_ipc::error_reports`): every runner running now is told at once
  /// (`Command::SetErrorReports`), without waiting for its answer, and every runner started from now on starts so.
  /// A runner that is starting is told once it is connected; one that cannot be reached misses nothing, since it
  /// starts with the setting the next time.
  pub fn set_error_reports(&self, reports: Option<&ErrorReports>) {
    let reports = reports.cloned();
    // Held while storing and telling the runners, so they end with the setting stored last.
    let _in_order = self.inner.report_updates.lock();
    *self.inner.reports.lock() = reports.clone();
    self.tell_every_runner(
      || Command::SetErrorReports {
        reports: reports.clone(),
      },
      |reply| matches!(reply, Reply::ErrorReportsSet),
      "the error reports setting",
    );
  }

  /// The project's browsers start with `browser`, this computer's Settings › Browser, from their next start: every
  /// runner attached now is told at once (`Command::SetBrowser`), without waiting for its answer, and every runner
  /// attaches with it from now on. Settings as they were are not sent again. Until this is called, a runner attaches
  /// with the settings in the application database beside this program.
  pub fn set_browser(&self, browser: &ComputerBrowser) {
    // Held while storing and telling the runners, so they end with the settings stored last.
    let _in_order = self.inner.browser_updates.lock();
    {
      let mut stored = self.inner.browser.lock();
      if stored.as_ref() == Some(browser) {
        return;
      }
      *stored = Some(browser.clone());
    }
    self.tell_every_runner(
      || Command::SetBrowser {
        browser: browser.clone(),
      },
      |reply| matches!(reply, Reply::BrowserSet),
      "Settings › Browser",
    );
  }

  /// Passes `workspace`'s Settings › Browser to the runners ([`Self::set_browser`]) now and whenever an application
  /// setting of the browser changes, until the future is dropped.
  pub async fn follow_browser_settings(self, workspace: Workspace) {
    let mut changes = workspace.outside_changes().subscribe_app();
    loop {
      let read = workspace.clone();
      match tokio::task::spawn_blocking(move || read.computer_browser()).await {
        Ok(browser) => self.set_browser(&browser),
        Err(error) => tracing::error!(%error, "reading Settings › Browser did not finish"),
      }
      if changes.changed().await.is_err() {
        return;
      }
    }
  }

  /// Sends the `command` each connected runner gets to it without waiting, and logs an answer that is not `taken`.
  fn tell_every_runner(&self, command: impl Fn() -> Command, taken: fn(&Reply) -> bool, setting: &'static str) {
    let answers: Vec<_> = self
      .inner
      .projects
      .lock()
      .iter()
      .filter_map(|(root, entry)| Some((root.clone(), entry.connection.as_ref()?.request(command()))))
      .collect();
    for (root, answer) in answers {
      self.inner.runtime.spawn(async move {
        // The task ended without answering, e.g. when the runner stopped meanwhile.
        match answer.await.unwrap_or(Err(RequestError::Lost)) {
          Ok(reply) if taken(&reply) => {}
          Ok(other) => tracing::error!(?other, setting, "unexpected reply to a setting"),
          Err(error) => {
            tracing::info!(project = %root.display(), %error, setting, "no runner took the setting now");
          }
        }
      });
    }
  }
}
