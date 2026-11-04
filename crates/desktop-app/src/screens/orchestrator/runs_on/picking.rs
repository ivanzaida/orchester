//! What the runs-on picker shows while it is open, and choosing in it. The rail's connection whose models show (the
//! one the Orchestrator runs on when the picker opens), what Search holds, whether every model shows, and where the
//! keys are: a column and the place in it. Choosing applies at once: the connection with what is kept on it or its
//! first model, a model with the effort when the model lists it, or an effort, published as the Orchestrator's next
//! version through the checks of its settings page (`orchester_workspace::orchestrator::settings::save_in`). Until
//! the version is read again, what was chosen shows chosen; a refused choice changes nothing and the picker says why.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{ElementRef, Signal},
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::settings::{Keep, OrchestratorSetup, Saving};

use super::{Publish, models::ModelItem, rail::Entry};

/// A column of the picker, where ← and → move the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
  Rail,
  Models,
  Effort,
}

impl DevtoolsInspectable for Part {}

/// The connection whose models show, as a signal holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Viewing(Option<ConnectionId>);

impl DevtoolsInspectable for Viewing {}

/// What was chosen since the picker opened, as a signal holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Applied(Option<OrchestratorSettings>);

impl DevtoolsInspectable for Applied {}

/// The picker's state while it is open.
#[derive(Clone)]
pub(super) struct Picking {
  /// The connection whose models show.
  viewing: Signal<Viewing>,
  /// What Search holds.
  pub(super) query: Signal<String>,
  /// *Show all N* was pressed.
  pub(super) all: Signal<bool>,
  /// The column the keys are in, and their place in it once an arrow moved them.
  pub(super) part: Signal<Part>,
  pub(super) at: Signal<Option<usize>>,
  /// What was chosen since the picker opened.
  applied: Signal<Applied>,
  seeded: Signal<bool>,
  /// Search takes the focus on the next pass.
  focus: Signal<bool>,
  pub(super) input: ElementRef,
}

/// What the keys move over: the rail's connections (whether each can be chosen), the models shown and the efforts,
/// `None` the CLI's own; and in each, where the one shown or chosen is.
pub(super) struct Lists {
  pub(super) rail: Vec<(ConnectionId, bool)>,
  pub(super) models: Vec<ModelItem>,
  pub(super) efforts: Vec<Option<String>>,
  pub(super) homes: [Option<usize>; 3],
}

impl Lists {
  pub(super) fn len(&self, part: Part) -> usize {
    match part {
      Part::Rail => self.rail.len(),
      Part::Models => self.models.len(),
      Part::Effort => self.efforts.len(),
    }
  }

  /// Where the keys start in `part`: on the one shown or chosen, else the first.
  pub(super) fn home(&self, part: Part) -> usize {
    let index = match part {
      Part::Rail => 0,
      Part::Models => 1,
      Part::Effort => 2,
    };
    self.homes[index].unwrap_or(0)
  }

  /// The columns the keys can be in, in their order: the rail and Effort only when they show something.
  pub(super) fn parts(&self) -> Vec<Part> {
    [Part::Rail, Part::Models, Part::Effort]
      .into_iter()
      .filter(|part| *part == Part::Models || self.len(*part) > 0)
      .collect()
  }
}

impl Picking {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      viewing: ctx.signal(Viewing::default()),
      query: ctx.signal(String::new()),
      all: ctx.signal(false),
      part: ctx.signal(Part::Models),
      at: ctx.signal(None),
      applied: ctx.signal(Applied::default()),
      seeded: ctx.signal(false),
      focus: ctx.signal(false),
      input: ElementRef::new(),
    }
  }

  /// The picker closed: it starts anew when it opens again.
  pub(super) fn forget(&self) {
    if self.seeded.get_untracked() {
      self.seeded.set(false);
    }
  }

  /// Once since the picker opened: the models of what the Orchestrator runs on show, else of the first connection it
  /// can run on, with nothing searched and Search focused.
  pub(super) fn seed(&self, setup: &OrchestratorSetup, entries: &[Entry]) {
    if self.seeded.get_untracked() {
      return;
    }
    let published = setup
      .settings
      .as_ref()
      .map(OrchestratorSettings::connection)
      .filter(|connection| entries.iter().any(|entry| entry.id == *connection));
    let viewing = published.or_else(|| entries.iter().find(|entry| entry.choosable).map(|entry| entry.id));
    self.viewing.set(Viewing(viewing));
    self.query.set(String::new());
    self.all.set(false);
    self.part.set(Part::Models);
    self.at.set(None);
    self.applied.set(Applied::default());
    self.focus.set(true);
    self.seeded.set(true);
  }

  /// The connection whose models show.
  pub(super) fn viewing(&self) -> Option<ConnectionId> {
    self.viewing.get().0
  }

  /// Search takes the focus when it was asked to.
  pub(super) fn take_focus(&self, ctx: &mut Ctx) {
    if self.focus.get() {
      ctx.focus(&self.input);
      self.focus.set(false);
    }
  }

  /// Search takes the focus with `typed` at its end: a key typed while the focus was elsewhere in the picker.
  pub(super) fn type_in(&self, typed: &str) {
    self.query.update(|query| query.push_str(typed));
    self.typed();
    self.focus.set(true);
  }

  /// What Search holds changed: the keys go to the models it finds, from the first.
  pub(super) fn typed(&self) {
    self.part.set(Part::Models);
    self.at.set(None);
  }

  /// Where the keys are in `part`, of `count` places: where an arrow moved them, else the first model found while
  /// Search holds text; `None` elsewhere.
  pub(super) fn cursor(&self, part: Part, count: usize) -> Option<usize> {
    if self.part.get() != part || count == 0 {
      return None;
    }
    let searched = part == Part::Models && !self.query.get().trim().is_empty();
    self.at.get().or(searched.then_some(0)).map(|at| at.min(count - 1))
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
  /// Shows `connection`'s models and runs on it, with what is kept on it, else its first model and the CLI's own
  /// effort; one with no model to start on only shows, for a model to be typed.
  pub(super) fn connection(&self, connection: ConnectionId) {
    let picking = &self.picking;
    if picking.viewing.get_untracked().0 != Some(connection) {
      picking.viewing.set(Viewing(Some(connection)));
      picking.query.set(String::new());
      picking.all.set(false);
    }
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

  /// Runs on `model` of the connection shown, with the effort chosen when `model` lists it.
  pub(super) fn model(&self, model: ModelId) {
    let Some(connection) = self.picking.viewing.get_untracked().0 else {
      return;
    };
    let efforts = efforts_for(&self.setup, connection, &model);
    let effort = self
      .chosen_on(connection)
      .and_then(|chosen| chosen.effort().map(str::to_owned))
      .filter(|effort| efforts.contains(effort));
    self.apply(OrchestratorSettings::new(connection, model, effort));
  }

  /// Runs on `effort` (`None`: the CLI's own) with the model chosen on the connection shown.
  pub(super) fn effort(&self, effort: Option<String>) {
    let Some(chosen) = self
      .picking
      .viewing
      .get_untracked()
      .0
      .and_then(|connection| self.chosen_on(connection))
    else {
      return;
    };
    self.apply(OrchestratorSettings::new(
      chosen.connection(),
      chosen.model().clone(),
      effort,
    ));
  }

  /// Every model of the connection shown shows.
  pub(super) fn show_all(&self) {
    self.picking.all.set(true);
  }

  /// What is chosen, when it is on `connection`.
  pub(super) fn chosen_on(&self, connection: ConnectionId) -> Option<&OrchestratorSettings> {
    self.chosen.as_ref().filter(|chosen| chosen.connection() == connection)
  }

  /// Publishes `settings` unless they are what is chosen.
  fn apply(&self, settings: OrchestratorSettings) {
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
