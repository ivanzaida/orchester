use std::sync::Arc;

use parking_lot::Mutex;

use super::{input_gate::InputGate, tools::ToolError};
use crate::{appearance::Appearance, navigation::Navigation, project::Projects};

/// What the window is, for the app's MCP tools, which are registered before the window's root exists:
/// a started session's window attaches its navigation and the projects its tabs show, a window without
/// a session the name of the startup screen it shows. Each attaches once. The window's appearance, which
/// `set_theme` and Settings › General both change, is attached before the root mounts.
#[derive(Clone, Default)]
pub struct WindowLink {
  linked: Arc<Mutex<Option<Linked>>>,
  /// Keeps MCP clients' window input from the window's person-only controls.
  gate: InputGate,
  appearance: Arc<Mutex<Option<Appearance>>>,
}

#[derive(Clone)]
enum Linked {
  Session {
    navigation: Navigation,
    projects: Projects,
  },
  /// The startup screen the window shows, by its `app_state` name.
  Startup(&'static str),
}

impl WindowLink {
  pub fn attach(&self, navigation: Navigation, projects: Projects) {
    *self.linked.lock() = Some(Linked::Session { navigation, projects });
  }

  pub fn attach_startup(&self, screen: &'static str) {
    *self.linked.lock() = Some(Linked::Startup(screen));
  }

  /// The window's colors, which the root hands to its screens.
  pub fn attach_appearance(&self, appearance: Appearance) {
    *self.appearance.lock() = Some(appearance);
  }

  /// The window's colors; `None` in a window that attached none (a test's), where Settings cannot choose them.
  pub fn appearance(&self) -> Option<Appearance> {
    self.appearance.lock().clone()
  }

  /// The navigation; [`ToolError::NotReady`] before the window attached it, [`ToolError::NoSession`]
  /// on a startup screen.
  pub fn navigation(&self) -> Result<Navigation, ToolError> {
    self.session().map(|(navigation, _)| navigation)
  }

  /// The projects the tabs show, like [`navigation`](Self::navigation).
  pub fn projects(&self) -> Result<Projects, ToolError> {
    self.session().map(|(_, projects)| projects)
  }

  /// The gate the window's person-only controls and the MCP server share.
  pub fn input_gate(&self) -> InputGate {
    self.gate.clone()
  }

  /// The startup screen the window shows, if it shows one.
  pub fn startup(&self) -> Option<&'static str> {
    match *self.linked.lock() {
      Some(Linked::Startup(screen)) => Some(screen),
      Some(Linked::Session { .. }) | None => None,
    }
  }

  fn session(&self) -> Result<(Navigation, Projects), ToolError> {
    match self.linked.lock().clone() {
      Some(Linked::Session { navigation, projects }) => Ok((navigation, projects)),
      Some(Linked::Startup(_)) => Err(ToolError::NoSession),
      None => Err(ToolError::NotReady),
    }
  }
}

/// Links are compared by identity: the root's props hold the one link of the window.
impl PartialEq for WindowLink {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.linked, &other.linked)
  }
}
