//! The window's requests to a project's browser (`orchester_ipc::browser::BrowserCommand`), handed to the project's
//! runner in the order they are made, their answers awaited off the UI thread. A request of the person's (navigating,
//! opening or closing a tab, answering a dialog, the files chosen for a page, the clipboard's shortcuts) the runner
//! refuses or loses is logged and said in the window until the next one is carried out; what the window asks by itself
//! or many times a second (which tab to screencast, the person's pointer and keys) is only logged.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_ipc::browser::{BrowserCommand, BrowserReply, BrowserTabId, PageCursor};
use orchester_workspace::runner::Runners;
use tokio::runtime::Handle;

/// The project whose browser restarts at the person's request (*Open in a browser window*, *Back to Orchester*), until
/// the runner answers. That takes seconds, while the window keeps showing what it showed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Restarting(pub Option<PathBuf>);

impl DevtoolsInspectable for Restarting {}

/// Sends the window's requests to the browser of one project.
#[derive(Clone)]
pub struct Commands {
  runners: Runners,
  runtime: Handle,
  root: Arc<PathBuf>,
  /// The person's last request was refused or lost.
  failed: Signal<bool>,
  restarting: Signal<Restarting>,
}

/// Commands to the same project are the same.
impl PartialEq for Commands {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for Commands {}

impl Commands {
  pub fn new(
    runners: Runners,
    runtime: Handle,
    root: PathBuf,
    (failed, restarting): (Signal<bool>, Signal<Restarting>),
  ) -> Self {
    Self {
      runners,
      runtime,
      root: Arc::new(root),
      failed,
      restarting,
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

  /// What the person asked could not be done before it became a request: said until the next one is carried out.
  pub fn say_failed(&self) {
    self.failed.set(true);
  }

  /// The mouse cursor's shape at `x`, `y` of `tab`: the arrow when the request is refused or lost, which is only logged
  /// as the person asked nothing.
  pub fn cursor_at(&self, tab: BrowserTabId, x: i32, y: i32) -> impl Future<Output = PageCursor> + Send + use<> {
    let answer = self.runners.browser(&self.root, BrowserCommand::CursorAt { tab, x, y });
    async move {
      match answer.await {
        Ok(BrowserReply::Cursor { cursor }) => cursor,
        Ok(other) => {
          tracing::debug!(?other, "the browser answered the cursor's shape with something else");
          PageCursor::Arrow
        }
        Err(error) => {
          tracing::debug!(%error, "the browser did not tell the cursor's shape");
          PageCursor::Arrow
        }
      }
    }
  }

  /// Runs `work` off the UI thread.
  pub fn spawn(&self, work: impl Future<Output = ()> + Send + 'static) {
    self.runtime.spawn(work);
  }

  fn dispatch(&self, command: BrowserCommand, said: bool, then: impl FnOnce(BrowserReply) + Send + 'static) {
    let what = command_name(&command);
    let restarts = matches!(command, BrowserCommand::OpenInWindow | BrowserCommand::BackToOrchester);
    if restarts {
      self.restarting.set(Restarting(Some(self.root.as_ref().clone())));
    }
    // Handed over here, not on the task below: the tasks start in no certain order, and the person's input reaches the
    // page in the order it was made.
    let answer = self.runners.browser(&self.root, command);
    let (root, failed, restarting) = (Arc::clone(&self.root), self.failed.clone(), self.restarting.clone());
    self.runtime.spawn(async move {
      let answer = answer.await;
      if restarts {
        restarting.set(Restarting::default());
      }
      match answer {
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
    BrowserCommand::ChooseFiles { .. } => "choose files",
    BrowserCommand::CopySelection { .. } => "copy selection",
    BrowserCommand::CutSelection { .. } => "cut selection",
    BrowserCommand::CursorAt { .. } => "cursor at",
    BrowserCommand::OpenInWindow => "open in window",
    BrowserCommand::BackToOrchester => "back to Orchester",
  }
}
