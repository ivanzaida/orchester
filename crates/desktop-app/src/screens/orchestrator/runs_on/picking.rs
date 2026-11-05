//! What the runs-on picker shows while it is open, and choosing in it. The rail's connection whose models show (the
//! one the Orchestrator runs on when the picker opens), what Search holds, the effort typed for an API provider, where
//! the models' list is scrolled, and where the keys are: a column and the place in it. Choosing applies at once: the
//! connection with what is kept on it or its first model, a model with the effort when the model lists it, or an
//! effort, published as the Orchestrator's next version through the checks of its settings page
//! (`orchester_workspace::orchestrator::settings::save_in`). Until the version is read again, what was chosen shows
//! chosen; a refused choice changes nothing and the picker says why, as it does for a connection that cannot be chosen.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::{ElementRef, Signal},
  layout::layout_kind::ScrollState,
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::settings::{Keep, OrchestratorSetup, Saving};

use super::{Publish, models, rail::Entry};

/// A column of the picker, where ← and → move the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
  Rail,
  Models,
  Effort,
}

impl DevtoolsInspectable for Part {}

/// A field of the picker that takes the focus on the next pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
  Stay,
  Search,
  Effort,
}

impl DevtoolsInspectable for Focus {}

/// A connection, as a signal holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Connection(Option<ConnectionId>);

impl DevtoolsInspectable for Connection {}

/// What was chosen since the picker opened, as a signal holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Applied(Option<OrchestratorSettings>);

impl DevtoolsInspectable for Applied {}

/// The picker's state while it is open.
#[derive(Clone)]
pub(super) struct Picking {
  /// The connection whose models show.
  viewing: Signal<Connection>,
  /// What Search holds.
  pub(super) query: Signal<String>,
  /// The effort typed for an API provider.
  pub(super) typed_effort: Signal<String>,
  /// The column the keys are in, and their place in it once an arrow moved them.
  pub(super) part: Signal<Part>,
  pub(super) at: Signal<Option<usize>>,
  /// What was chosen since the picker opened.
  applied: Signal<Applied>,
  /// A connection the keys tried to choose that cannot be chosen: the picker says why.
  unavailable: Signal<Connection>,
  seeded: Signal<bool>,
  /// The chosen model is scrolled into view on the next pass.
  reveal: Signal<bool>,
  focus: Signal<Focus>,
  pub(super) search: ElementRef,
  pub(super) effort_field: ElementRef,
  /// Where the models' list is scrolled.
  pub(super) scroll: ScrollState,
}

impl Picking {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      viewing: ctx.signal(Connection::default()),
      query: ctx.signal(String::new()),
      typed_effort: ctx.signal(String::new()),
      part: ctx.signal(Part::Models),
      at: ctx.signal(None),
      applied: ctx.signal(Applied::default()),
      unavailable: ctx.signal(Connection::default()),
      seeded: ctx.signal(false),
      reveal: ctx.signal(false),
      focus: ctx.signal(Focus::Stay),
      search: ElementRef::new(),
      effort_field: ElementRef::new(),
      scroll: ScrollState::new(),
    }
  }

  /// The picker closed: it starts anew when it opens again.
  pub(super) fn forget(&self) {
    if self.seeded.get_untracked() {
      self.seeded.set(false);
    }
  }

  /// Once since the picker opened: the models of what the Orchestrator runs on show, scrolled to the chosen one, else
  /// of the first connection it can run on, with nothing searched and Search focused.
  pub(super) fn seed(&self, setup: &OrchestratorSetup, entries: &[Entry]) {
    if self.seeded.get_untracked() {
      return;
    }
    let published = setup
      .settings
      .as_ref()
      .filter(|settings| entries.iter().any(|entry| entry.id == settings.connection()));
    let viewing = published
      .map(OrchestratorSettings::connection)
      .or_else(|| entries.iter().find(|entry| entry.choosable).map(|entry| entry.id));
    self.viewing.set(Connection(viewing));
    self.query.set(String::new());
    self.typed_effort.set(
      published
        .and_then(OrchestratorSettings::effort)
        .unwrap_or_default()
        .to_owned(),
    );
    self.part.set(Part::Models);
    self.at.set(None);
    self.applied.set(Applied::default());
    self.unavailable.set(Connection::default());
    self.reveal.set(true);
    self.focus.set(Focus::Search);
    self.seeded.set(true);
  }

  /// The connection whose models show.
  pub(super) fn viewing(&self) -> Option<ConnectionId> {
    self.viewing.get().0
  }

  /// A connection the keys tried to choose that cannot be chosen.
  pub(super) fn unavailable(&self) -> Option<ConnectionId> {
    self.unavailable.get().0
  }

  /// The keys tried to choose `connection`, which cannot be chosen.
  pub(super) fn refuse(&self, connection: ConnectionId) {
    self.unavailable.set(Connection(Some(connection)));
  }

  /// The field asked for takes the focus, and the chosen model at `chosen` in the list scrolls into view once.
  pub(super) fn settle(&self, ctx: &mut Ctx, chosen: Option<usize>) {
    match self.focus.get() {
      Focus::Stay => {}
      Focus::Search => ctx.focus(&self.search),
      Focus::Effort => ctx.focus(&self.effort_field),
    }
    if self.focus.get_untracked() != Focus::Stay {
      self.focus.set(Focus::Stay);
    }
    if self.reveal.get() {
      if let Some(chosen) = chosen {
        models::reveal(&self.scroll, chosen);
      }
      self.reveal.set(false);
    }
  }

  /// The keys are on the typed effort (`on_field`) or elsewhere: its field or Search takes the focus.
  pub(super) fn focus_for(&self, on_field: bool) {
    let target = if on_field { Focus::Effort } else { Focus::Search };
    let focused = if on_field {
      self.effort_field.focused()
    } else {
      self.search.focused()
    };
    if !focused {
      self.focus.set(target);
    }
  }

  /// Search takes the focus with `typed` at its end: a key typed while the focus was elsewhere in the picker.
  pub(super) fn type_in(&self, typed: &str) {
    self.query.update(|query| query.push_str(typed));
    self.typed();
    self.focus.set(Focus::Search);
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
    picking.unavailable.set(Connection::default());
    if picking.viewing.get_untracked().0 != Some(connection) {
      picking.viewing.set(Connection(Some(connection)));
      picking.query.set(String::new());
      picking.scroll.scroll_to_top_pending();
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
      picking
        .typed_effort
        .set(settings.effort().unwrap_or_default().to_owned());
      self.apply(settings);
    }
  }

  /// Runs on `model` of the connection shown, with the effort chosen when `model` lists it, or the effort typed on an
  /// API provider.
  pub(super) fn model(&self, model: ModelId) {
    let Some(connection) = self.picking.viewing.get_untracked().0 else {
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
    let effort = effort
      .map(|effort| effort.trim().to_owned())
      .filter(|effort| !effort.is_empty());
    self.picking.typed_effort.set(effort.clone().unwrap_or_default());
    self.apply(OrchestratorSettings::new(
      chosen.connection(),
      chosen.model().clone(),
      effort,
    ));
  }

  /// Runs on the effort typed for an API provider; nothing typed is the CLI's own.
  pub(super) fn typed_effort(&self) {
    self.effort(Some(self.picking.typed_effort.get_untracked()));
  }

  /// What is chosen, when it is on `connection`.
  pub(super) fn chosen_on(&self, connection: ConnectionId) -> Option<&OrchestratorSettings> {
    self.chosen.as_ref().filter(|chosen| chosen.connection() == connection)
  }

  /// `connection` lists no efforts of its own: its effort is typed (an API provider).
  pub(super) fn typed_only(&self, connection: ConnectionId) -> bool {
    matches!(
      self.setup.runs_on(connection),
      Some(orchester_workspace::orchestrator::runs_on::RunsOn::ApiProvider { .. })
    )
  }

  /// Publishes `settings` unless they are what is chosen.
  fn apply(&self, settings: OrchestratorSettings) {
    self.picking.unavailable.set(Connection::default());
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
