//! UI state; graph edits and their validation remain the core's. The panel beside the graph is the route's
//! ([`Panel`]), so Back and Forward step through the panels a person opened.
use std::{collections::BTreeMap, fmt};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{Pipeline, PipelineEdit, Step, StepKey, VersionNumber},
};
use orchester_workspace::{messages::Message, pipelines::data::OpenedPipeline};

pub use crate::routes::PipelinePanel as Panel;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
};

/// Opens the editor's panels by its route. Called outside a [`State`] update: a move can ask the leave guard,
/// which reads the state.
#[derive(Clone)]
pub struct Panels {
  navigation: Navigation,
  project: ProjectId,
  pipeline: ResourceId,
}
impl Panels {
  pub const fn new(navigation: Navigation, project: ProjectId, pipeline: ResourceId) -> Self {
    Self {
      navigation,
      project,
      pipeline,
    }
  }
  /// A panel the person chose, as a new entry of the tab's history; of the published version viewed, if
  /// any, but the publish review, which is the draft's.
  pub fn choose(&self, panel: Panel) {
    let viewed = self.viewed().filter(|_| panel != Panel::Publish);
    self.navigation.go(self.route(panel, viewed));
  }
  /// A published version the person picked in the history, as a new entry of the tab's history.
  pub fn view(&self, version: VersionNumber) {
    self.navigation.go(self.route(Panel::History, Some(version)));
  }
  /// *Cancel*, the close button, a removed step and a publish done: back to the summary of what is shown,
  /// the draft or the version viewed, leaving no entry for the panel closed.
  pub fn close(&self) {
    self.navigation.close_panel(self.route(Panel::Pipeline, self.viewed()));
  }
  /// *Return to draft* and a restore done: back to the draft's summary, leaving no entry for the version.
  pub fn to_draft(&self) {
    self.navigation.close_panel(self.route(Panel::Pipeline, None));
  }
  /// The published version the route views, if it is this editor's.
  fn viewed(&self) -> Option<VersionNumber> {
    match self.navigation.route_now() {
      Route::Project(project, ProjectPage::PipelineVersion(pipeline, version, _))
        if project == self.project && pipeline == self.pipeline =>
      {
        Some(version)
      }
      _ => None,
    }
  }
  /// Whether `route` is this editor's, on any panel: moving between panels leaves no edits behind.
  pub fn holds(&self, route: &Route) -> bool {
    matches!(route, Route::Project(project, ProjectPage::Pipeline(pipeline, _) | ProjectPage::PipelineVersion(pipeline, ..))
      if *project == self.project && *pipeline == self.pipeline)
  }
  fn route(&self, panel: Panel, version: Option<VersionNumber>) -> Route {
    Route::Project(
      self.project,
      match version {
        Some(version) => ProjectPage::PipelineVersion(self.pipeline, version, panel),
        None => ProjectPage::Pipeline(self.pipeline, panel),
      },
    )
  }
}
impl PartialEq for Panels {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.pipeline == other.pipeline
  }
}
impl Eq for Panels {}
impl fmt::Debug for Panels {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Panels")
      .field("project", &self.project)
      .field("pipeline", &self.pipeline)
      .finish_non_exhaustive()
  }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
  pub opened: OpenedPipeline,
  pub name: orchester_core::name::Name,
  pub edit: PipelineEdit,
  /// The panel the route shows, as the editor last rendered it; changed only through [`Panels`].
  pub panel: Panel,
  pub panels: Panels,
  pub errors: BTreeMap<String, Message>,
  pub inputs: BTreeMap<String, String>,
  /// The published version shown read-only, as the route names it ([`Panels::view`]).
  pub viewing: Option<Pipeline>,
  pub viewing_version: Option<VersionNumber>,
  pub epoch: u64,
  pub comparing: bool,
  pub restore_requested: bool,
  pub published_notice: Option<orchester_core::resource::VersionNumber>,
}
impl DevtoolsInspectable for Editing {}
impl Editing {
  pub fn new(opened: OpenedPipeline, (panels, panel): (Panels, Panel)) -> Self {
    Self {
      edit: PipelineEdit::new(opened.body.clone()),
      name: orchester_core::name::Name::new(&opened.name).expect("stored resource names are valid"),
      opened,
      panel,
      panels,
      errors: BTreeMap::new(),
      inputs: BTreeMap::new(),
      viewing: None,
      viewing_version: None,
      epoch: 0,
      comparing: false,
      restore_requested: false,
      published_notice: None,
    }
  }
  pub fn dirty(&self) -> bool {
    self.name.as_str() != self.opened.name.as_ref() || self.edit.validate().as_ref() != Ok(&self.opened.body)
  }
  pub fn shown(&self) -> PipelineEdit {
    self
      .viewing
      .clone()
      .map(PipelineEdit::new)
      .unwrap_or_else(|| self.edit.clone())
  }
  pub fn change(&mut self, key: &StepKey, change: impl FnOnce(&mut Step)) {
    self.edit.update(key, change);
  }
  pub fn field<T>(&mut self, id: &str, value: Result<T, Message>, apply: impl FnOnce(&mut Self, T)) {
    match value {
      Ok(value) => {
        self.errors.remove(id);
        apply(self, value);
      }
      Err(error) => {
        self.errors.insert(id.to_owned(), error);
      }
    }
  }
  /// Takes the version viewed into the draft; the caller then returns to the draft ([`Panels::to_draft`]).
  pub fn restore(&mut self) {
    if let Some(body) = self.viewing.take() {
      self.edit = PipelineEdit::new(body);
      self.viewing_version = None;
      self.comparing = false;
      self.restore_requested = false;
      self.epoch += 1;
      self.errors.clear();
      self.inputs.clear();
    }
  }
  pub fn saved(&mut self, opened: OpenedPipeline) {
    let latest = opened.latest.as_ref().map(|v| v.number);
    if latest != self.opened.latest.as_ref().map(|v| v.number) {
      self.published_notice = latest;
    }
    self.opened = opened;
  }
}
pub type State = Signal<Editing>;
