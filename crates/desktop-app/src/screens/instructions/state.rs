//! Presentation state for Instructions fePhH; instruction validation stays in workspace. The scope tab, the
//! rule shown and the panel beside it are the route's ([`View`]), so Back and Forward step through them.
use std::sync::Arc;

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{id::ProjectId, resource::InstructionScope};
use orchester_workspace::instructions::data::InstructionEntry;

pub use crate::routes::{InstructionsPanel as Panel, InstructionsTab as Tab, InstructionsView as View};
use crate::{
  navigation::{Navigation, link::Open, press::OpenIn},
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
}
impl Selection {
  /// The view the route shows now.
  pub fn now(&self) -> View {
    match self.navigation.route_now() {
      Route::Project(_, ProjectPage::Instructions(view)) => view,
      _ => View::default(),
    }
  }
  /// The rule a page such as the import was opened from, for its *Cancel* and back links; the first tab's
  /// when it was opened from elsewhere.
  pub fn opened_from(&self) -> View {
    match self.navigation.tabs_now().active().behind() {
      Some(Route::Project(project, ProjectPage::Instructions(view))) if *project == self.project => *view,
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
  /// What opens `view`, a place of its own, where a press asks (`navigation::link`): in the active tab as
  /// [`open`](Self::open) does, in a new tab as it is.
  pub fn to(&self, view: View) -> Open {
    let selection = self.clone();
    Arc::new(move |at| match at {
      OpenIn::ActiveTab => selection.open(view),
      at => selection.navigation.open(selection.route(view), at),
    })
  }
  /// Goes to `view`, as a new entry of the tab's history.
  pub fn show(&self, view: View) {
    self.navigation.go(self.route(view));
  }
  /// *Cancel*: shows `view`, leaving no entry for the page it closes.
  pub fn close(&self, view: View) {
    self.navigation.close_panel(self.route(view));
  }
  /// The rule a new rule became once saved, in the new rule's place.
  pub fn saved(&self, view: View) {
    self.navigation.replace(self.route(view));
  }
  /// Whether `route` shows the rule shown now, whatever its panel: a move there keeps the rule's edits.
  pub fn holds(&self, route: &Route) -> bool {
    let rule = self.now().with_panel(Panel::Hidden);
    matches!(route, Route::Project(project, ProjectPage::Instructions(view))
      if *project == self.project && view.with_panel(Panel::Hidden) == rule)
  }
  fn route(&self, view: View) -> Route {
    Route::Project(self.project, ProjectPage::Instructions(view))
  }
}
/// The panel beside the rule, as the route shows it; the editor keeps it in step with the route.
#[derive(Clone)]
pub struct SidePanel {
  shown: Signal<Panel>,
  selection: Selection,
}
impl SidePanel {
  pub const fn new(shown: Signal<Panel>, selection: Selection) -> Self {
    Self { shown, selection }
  }
  /// The panel shown; reading it in a render re-renders it when the panel changes.
  pub fn get(&self) -> Panel {
    self.shown.get()
  }
  /// Whether `other` is the same panel of the same editor.
  pub fn same(&self, other: &Self) -> bool {
    self.shown.id() == other.shown.id()
  }
  /// A panel the person chose, as a new entry of the tab's history.
  pub fn choose(&self, panel: Panel) {
    self.selection.show(self.selection.now().with_panel(panel));
  }
  /// The close button, *Back* and Escape from the full prompt: shows `panel` without leaving an entry for
  /// the one closed.
  pub fn close(&self, panel: Panel) {
    self.selection.close(self.selection.now().with_panel(panel));
  }
  /// Takes in the panel the route shows now.
  pub fn follow(&self, panel: Panel) {
    if self.shown.get_untracked() != panel {
      self.shown.set(panel);
    }
  }
}
