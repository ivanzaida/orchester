//! Choosing in the runs-on picker. The shared picker (`ui::runs_on_picker`) applies nothing itself: what it hands back
//! is applied here at once, the connection with what is kept on it or its first model, a model with the effort when the
//! model lists it, or an effort, published as the Orchestrator's next version through the checks of its settings page
//! (`orchester_workspace::orchestrator::settings::save_in`). Until the version is read again, what was chosen shows
//! chosen; a refused choice changes nothing and the picker says why.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::{
  runs_on::RunsOn,
  settings::{Keep, OrchestratorSetup, Saving},
};

use super::Publish;

/// What was chosen since the picker opened, as a signal holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Applied(Option<OrchestratorSettings>);

impl DevtoolsInspectable for Applied {}

/// What the picker keeps while it is open.
#[derive(Clone)]
pub(super) struct Picking {
  /// What was chosen since the picker opened.
  applied: Signal<Applied>,
  /// A model was typed that no connection takes (a name has no whitespace and at most 200 characters).
  rejected: Signal<bool>,
}

impl Picking {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      applied: ctx.signal(Applied::default()),
      rejected: ctx.signal(false),
    }
  }

  /// The picker closed: it starts anew when it opens again.
  pub(super) fn forget(&self) {
    if self.applied.get_untracked().0.is_some() {
      self.applied.set(Applied::default());
    }
    if self.rejected.get_untracked() {
      self.rejected.set(false);
    }
  }

  /// What the picker shows chosen: what was chosen since it opened unless publishing it `failed`, else the published
  /// version.
  pub(super) fn chosen(&self, setup: &OrchestratorSetup, failed: bool) -> Option<OrchestratorSettings> {
    self
      .applied
      .get()
      .0
      .filter(|_| !failed)
      .or_else(|| setup.settings.clone())
  }

  /// Something was chosen since the picker opened: why publishing it failed is the picker's to say.
  pub(super) fn chose(&self) -> bool {
    self.applied.get().0.is_some()
  }

  /// A model typed was not a model name.
  pub(super) fn rejected(&self) -> bool {
    self.rejected.get()
  }
}

/// What choosing in the picker does, for the project at `root`, `chosen` what it shows chosen.
#[derive(Clone)]
pub(super) struct Chooser {
  pub(super) picking: Picking,
  pub(super) setup: OrchestratorSetup,
  pub(super) chosen: Option<OrchestratorSettings>,
  pub(super) publish: Publish,
  pub(super) root: std::path::PathBuf,
}

impl Chooser {
  /// Runs on `connection`, with what is kept on it, else its first model and the CLI's own effort; one with no model to
  /// start on is only shown by the picker, for a model to be typed.
  pub(super) fn connection(&self, connection: ConnectionId) {
    if self.chosen_on(connection).is_some() {
      return;
    }
    let settings = self
      .setup
      .settings
      .clone()
      .filter(|settings| settings.connection() == connection)
      .or_else(|| self.setup.choosing(connection));
    if let Some(settings) = settings {
      self.apply(settings);
    }
  }

  /// Runs on `model` of `connection`, with the effort chosen when `model` lists it, or the one typed on an API
  /// provider.
  pub(super) fn model(&self, connection: ConnectionId, model: &str) {
    let Ok(model) = ModelId::new(model) else {
      self.picking.rejected.set(true);
      return;
    };
    let typed = self.typed_only(connection);
    let efforts = efforts_for(&self.setup, connection, &model);
    let effort = self
      .chosen_on(connection)
      .and_then(|chosen| chosen.effort().map(str::to_owned))
      .filter(|effort| typed || efforts.contains(effort));
    self.apply(OrchestratorSettings::new(connection, model, effort));
  }

  /// Runs on `effort` (`None`: the CLI's own) with the model chosen.
  pub(super) fn effort(&self, effort: Option<&str>) {
    let Some(chosen) = &self.chosen else {
      return;
    };
    let effort = effort
      .map(str::trim)
      .filter(|effort| !effort.is_empty())
      .map(str::to_owned);
    self.apply(OrchestratorSettings::new(
      chosen.connection(),
      chosen.model().clone(),
      effort,
    ));
  }

  /// What is chosen, when it is on `connection`.
  fn chosen_on(&self, connection: ConnectionId) -> Option<&OrchestratorSettings> {
    self.chosen.as_ref().filter(|chosen| chosen.connection() == connection)
  }

  /// `connection` lists no efforts of its own: its effort is typed (an API provider).
  fn typed_only(&self, connection: ConnectionId) -> bool {
    matches!(self.setup.runs_on(connection), Some(RunsOn::ApiProvider { .. }))
  }

  /// Publishes `settings` unless they are what is chosen.
  fn apply(&self, settings: OrchestratorSettings) {
    self.picking.rejected.set(false);
    if self.chosen.as_ref() == Some(&settings) {
      return;
    }
    self.picking.applied.set(Applied(Some(settings.clone())));
    self.publish.run(Saving {
      root: self.root.clone(),
      settings,
      keep: Keep::Publish,
    });
  }
}

/// The efforts `connection` lists for `model`, else for any of its models: what a model it does not list may take.
pub(super) fn efforts_for(setup: &OrchestratorSetup, connection: ConnectionId, model: &ModelId) -> Vec<String> {
  let efforts = setup.efforts(connection, model);
  if efforts.is_empty() {
    setup.any_efforts(connection)
  } else {
    efforts
  }
}
