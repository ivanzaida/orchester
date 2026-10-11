//! Local selection, inspector mode and form choices; no credential values.
use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{
  id::SecretId,
  secret::{Environment, SecretKind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Panel {
  Inspect,
  Add,
  Replace,
  Archive,
  Usage,
}
impl DevtoolsInspectable for Panel {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selected(pub Option<SecretId>);
impl DevtoolsInspectable for Selected {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnvironmentChoice(pub Option<Environment>);
impl DevtoolsInspectable for EnvironmentChoice {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KindChoice(pub SecretKind);
impl DevtoolsInspectable for KindChoice {}

#[derive(Clone)]
pub(super) struct View {
  pub selected: Signal<Selected>,
  pub panel: Signal<Panel>,
  pub environment: Signal<EnvironmentChoice>,
  pub revision: Signal<u64>,
}
impl View {
  /// A form may choose an environment outside the list's filter.
  pub fn reveal(&self, id: SecretId, environment: Environment) {
    if self.environment.get().0.is_some_and(|current| current != environment) {
      self.environment.set(EnvironmentChoice(Some(environment)));
    }
    self.inspect(id);
  }
  pub fn inspect(&self, id: SecretId) {
    self.selected.set(Selected(Some(id)));
    self.panel.set(Panel::Inspect);
  }
}
