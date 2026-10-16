//! The page's tab, filter, chosen secret and panel live in its route ([`SecretsView`]); here are the moves
//! between them and the form choices. No credential values.
use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{
  id::{ProjectId, SecretId},
  secret::{Environment, SecretKind},
};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, SecretsPanel, SecretsTab, SecretsView},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnvironmentChoice(pub Option<Environment>);
impl DevtoolsInspectable for EnvironmentChoice {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KindChoice(pub SecretKind);
impl DevtoolsInspectable for KindChoice {}

#[derive(Clone)]
pub(super) struct View {
  pub project: ProjectId,
  pub navigation: Navigation,
  /// The list's environment filter as its select holds it; a choice goes to the route.
  pub environment: Signal<EnvironmentChoice>,
  pub revision: Signal<u64>,
}
impl View {
  /// What the tab shows now: rows and buttons keep the closures they were mounted with, so a press reads
  /// the route as it lands.
  fn now(&self) -> SecretsView {
    match self.navigation.route_now() {
      Route::Project(_, ProjectPage::Secrets(view)) => view,
      _ => SecretsView::default(),
    }
  }
  fn route(&self, view: SecretsView) -> Route {
    Route::Project(self.project, ProjectPage::Secrets(view))
  }
  /// A tab the person chose, with the list's inspector.
  pub fn tab(&self, tab: SecretsTab) {
    let view = SecretsView {
      tab,
      panel: SecretsPanel::Inspect,
      ..self.now()
    };
    self.navigation.go(self.route(view));
  }
  /// A panel the person opened: *New secret*, or *Replace value* or *Archive* of `secret`.
  pub fn open(&self, panel: SecretsPanel, secret: Option<SecretId>) {
    let now = self.now();
    let view = SecretsView {
      panel,
      secret: secret.or(now.secret),
      ..now
    };
    self.navigation.go(self.route(view));
  }
  /// *Cancel*: back to the inspector, leaving no entry for the closed panel.
  pub fn close_panel(&self) {
    let view = SecretsView {
      panel: SecretsPanel::Inspect,
      ..self.now()
    };
    self.navigation.close_panel(self.route(view));
  }
  /// An environment the person chose in the filter.
  pub fn filter(&self, environment: Option<Environment>) {
    let now = self.now();
    if now.environment != environment {
      self.navigation.go(self.route(SecretsView { environment, ..now }));
    }
  }
  /// A secret the person chose, in its inspector; a filter that hides it shows its environment.
  pub fn reveal(&self, id: SecretId, environment: Environment) {
    self.navigation.go(self.route(self.revealed(id, environment)));
  }
  /// The secret a form saved, in the form's place: the route corrected, no place chosen.
  pub fn saved(&self, id: SecretId, environment: Environment) {
    self.navigation.replace(self.route(self.revealed(id, environment)));
  }
  /// The inspector again once an archive is done, in the panel's place.
  pub fn settled(&self) {
    let view = SecretsView {
      panel: SecretsPanel::Inspect,
      ..self.now()
    };
    self.navigation.replace(self.route(view));
  }
  fn revealed(&self, id: SecretId, environment: Environment) -> SecretsView {
    let now = self.now();
    SecretsView {
      environment: now.environment.map(|_| environment),
      secret: Some(id),
      panel: SecretsPanel::Inspect,
      ..now
    }
  }
}
