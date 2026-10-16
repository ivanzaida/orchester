//! Presentation state for Instructions fePhH; instruction validation stays in workspace. The scope tab and
//! the rule shown are the route's ([`View`]), so Back and Forward step through them.
use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{id::ProjectId, resource::InstructionScope};
use orchester_workspace::instructions::data::InstructionEntry;

pub use crate::routes::{InstructionsTab as Tab, InstructionsView as View};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
};

/// What the screen says about a scope tab.
impl Tab {
  pub const fn of(scope: &InstructionScope) -> Self {
    match scope {
      InstructionScope::Project => Self::Project,
      InstructionScope::AgentRole { .. } => Self::Roles,
      InstructionScope::Pipeline { .. } => Self::Pipelines,
      InstructionScope::Step { .. } => Self::Steps,
    }
  }
  pub const fn key(self) -> &'static str {
    match self {
      Self::Project => "instructions.projectRules",
      Self::Roles => "instructions.agentRoles",
      Self::Pipelines => "instructions.pipelineOverrides",
      Self::Steps => "instructions.stepOverrides",
    }
  }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved(pub Option<InstructionEntry>);
impl DevtoolsInspectable for Saved {}
#[derive(Clone)]
pub struct Selection {
  pub project: ProjectId,
  pub navigation: Navigation,
  /// A view chosen while the rule shown has edits it must not lose, until the person answers.
  pub pending: Signal<Option<View>>,
  pub created: Signal<Saved>,
  pub dirty: Signal<bool>,
  pub busy: Signal<bool>,
  pub full: Signal<bool>,
}
impl Selection {
  /// The view the route shows now.
  pub fn now(&self) -> View {
    match self.navigation.route_now() {
      Route::Project(_, ProjectPage::Instructions(view)) => view,
      _ => View::default(),
    }
  }
  /// A view the person chose; held for the leave question while the rule shown has unsaved edits.
  pub fn open(&self, view: View) {
    if self.dirty.get_untracked() || self.busy.get_untracked() {
      self.pending.set(Some(view));
    } else {
      self.show(view);
    }
  }
  /// Goes to `view`, as a new entry of the tab's history.
  pub fn show(&self, view: View) {
    self.full.set(false);
    self.navigation.go(self.route(view));
  }
  /// *Cancel*: shows `view`, leaving no entry for the page it closes.
  pub fn close(&self, view: View) {
    self.full.set(false);
    self.navigation.close_panel(self.route(view));
  }
  /// The rule a new rule became once saved, in the new rule's place.
  pub fn saved(&self, view: View) {
    self.navigation.replace(self.route(view));
  }
  fn route(&self, view: View) -> Route {
    Route::Project(self.project, ProjectPage::Instructions(view))
  }
}
