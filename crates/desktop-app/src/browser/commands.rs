//! The window's requests to a project's browser (`orchester_ipc::browser::BrowserCommand`), sent to the project's
//! runner off the UI thread. A request of the person's (navigating, opening or closing a tab, answering a dialog) the
//! runner refuses or loses is logged and said in the window until the next one is carried out; what the window asks
//! by itself or many times a second (which tab to screencast, the person's pointer and keys) is only logged.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_ipc::browser::{BrowserCommand, BrowserReply};
use orchester_workspace::runner::Runners;
use tokio::runtime::Handle;

/// Sends the window's requests to the browser of one project.
#[derive(Clone)]
pub struct Commands {
  runners: Runners,
  runtime: Handle,
  root: Arc<PathBuf>,
  /// The person's last request was refused or lost.
  failed: Signal<bool>,
}

/// Commands to the same project are the same.
impl PartialEq for Commands {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for Commands {}

impl Commands {
  pub fn new(runners: Runners, runtime: Handle, root: PathBuf, failed: Signal<bool>) -> Self {
    Self {
      runners,
      runtime,
      root: Arc::new(root),
      failed,
    }
  }

  /// The same, to the browser of the project at `root`.
  pub fn for_project(&self, root: &Path) -> Self {
    Self {
      root: Arc::new(root.to_path_buf()),
      ..self.clone()
    }
  }

  /// Sends the person's `command`; `then` runs with the reply once it was carried out.
  pub fn send_then(&self, command: BrowserCommand, then: impl FnOnce(BrowserReply) + Send + 'static) {
    self.dispatch(command, true, then);
  }

  /// Sends the person's `command`.
  pub fn send(&self, command: BrowserCommand) {
    self.dispatch(command, true, |_| {});
  }

  /// Sends a `command` the window makes by itself or many times a second: a refusal is logged, not said.
  pub fn send_quietly(&self, command: BrowserCommand) {
    self.dispatch(command, false, |_| {});
  }

  fn dispatch(&self, command: BrowserCommand, said: bool, then: impl FnOnce(BrowserReply) + Send + 'static) {
    let (runners, root, failed) = (self.runners.clone(), Arc::clone(&self.root), self.failed.clone());
    self.runtime.spawn(async move {
      let what = command_name(&command);
      match runners.browser(&root, command).await {
        Ok(reply) => {
          if said && failed.get_untracked() {
            failed.set(false);
          }
          then(reply);
        }
        Err(error) => {
          tracing::warn!(project = %root.display(), what, %error, "the browser refused or lost a request");
          if said {
            failed.set(true);
          }
        }
      }
    });
  }
}

/// The request's name for the log, without what it carries (an address, typed text).
const fn command_name(command: &BrowserCommand) -> &'static str {
  match command {
    BrowserCommand::Subscribe => "subscribe",
    BrowserCommand::Show { .. } => "show",
    BrowserCommand::StopShowing => "stop showing",
    BrowserCommand::Input { .. } => "input",
    BrowserCommand::Navigate { .. } => "navigate",
    BrowserCommand::OpenTab { .. } => "open tab",
    BrowserCommand::CloseTab { .. } => "close tab",
    BrowserCommand::AnswerDialog { .. } => "answer dialog",
    BrowserCommand::ChooseOption { .. } => "choose option",
    BrowserCommand::OpenInWindow => "open in window",
    BrowserCommand::BackToOrchester => "back to Orchester",
  }
}
