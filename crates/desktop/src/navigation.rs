//! Which screen the window shows, shared with screens through lurq context.

use std::{path::PathBuf, sync::Arc};

use lurq::core::Signal;

use crate::{
  messages::{self, Message},
  workspace::{Landing, OpenProject, Workspace},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
  /// First launch (`ZBoBm`).
  Welcome,
  /// Project & repository setup (`W6uXiL`).
  Setup,
  /// Project overview inside the project shell (`Twtqc`).
  Project(OpenProject),
  /// The most recently used project could not be opened.
  Unavailable {
    name: Arc<str>,
    repo_root: PathBuf,
    reason: Message,
  },
}

impl lurq::app::component::DevtoolsInspectable for Screen {}

impl From<Landing> for Screen {
  fn from(landing: Landing) -> Self {
    match landing {
      Landing::FirstLaunch => Self::Welcome,
      Landing::Project(project) => Self::Project(project),
      Landing::Unavailable { project, error } => {
        tracing::error!(%error, repo_root = %project.repo_root.display(), "cannot open the most recent project");
        Self::Unavailable {
          name: project.name.as_str().into(),
          repo_root: project.repo_root,
          reason: messages::store_error(&error),
        }
      }
    }
  }
}

/// Provided by the root: the workspace and the screen the user navigated to. `None` means the
/// screen chosen at startup.
#[derive(Clone)]
pub struct Navigation {
  workspace: Workspace,
  screen: Signal<Option<Screen>>,
}

impl Navigation {
  pub fn new(workspace: Workspace, screen: Signal<Option<Screen>>) -> Self {
    Self { workspace, screen }
  }

  pub fn workspace(&self) -> &Workspace {
    &self.workspace
  }

  pub fn show(&self, screen: Screen) {
    self.screen.set(Some(screen));
  }

  /// Back to the screen chosen at startup.
  pub fn back_to_start(&self) {
    self.screen.set(None);
  }
}
