//! UI state; step edits and their validation remain the core's. What the page shows is the route's ([`View`]):
//! its tab and what the Steps column shows, so Back and Forward step through the places a person opened.
use std::{collections::BTreeMap, fmt};

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::{Pipeline, PipelineEdit, Step, StepKey, VersionNumber},
};
use orchester_workspace::{messages::Message, pipelines::data::OpenedPipeline};

pub use crate::routes::{PipelineTab as Tab, PipelineView as View, StepPage};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
};

/// Opens the page's places by its route. Called outside a [`State`] update: a move can ask the leave guard, which
/// reads the state.
#[derive(Clone)]
pub struct Views {
  navigation: Navigation,
  project: ProjectId,
  pipeline: ResourceId,
}
impl Views {
  pub const fn new(navigation: Navigation, project: ProjectId, pipeline: ResourceId) -> Self {
    Self {
      navigation,
      project,
      pipeline,
    }
  }
  /// A place the person chose, as a new entry of the tab's history; of the published version viewed, if any, but
  /// the publish review, which is the draft's.
  pub fn choose(&self, view: View) {
    let viewed = self.viewed().filter(|_| view != View::Publish);
    self.navigation.go(self.route(view, viewed));
  }
  /// A tab the person chose: the draft's, leaving the version viewed.
  pub fn tab(&self, view: View) {
    self.navigation.go(self.route(view, None));
  }
  /// A published version the person picked in Versions, as a new entry of the tab's history.
  pub fn view(&self, version: VersionNumber) {
    self.navigation.go(self.route(View::Versions, Some(version)));
  }
  /// *Cancel*, a removed step and a publish done: back to the summary of what is shown, the draft's or the
  /// version's, leaving no entry for the place closed.
  pub fn close(&self) {
    let view = if self.viewed().is_some() { View::Versions } else { View::Steps };
    self.navigation.close_panel(self.route(view, self.viewed()));
  }
  /// *Back to editing* and a restore done: back to the draft's steps, leaving no entry for the version.
  pub fn to_draft(&self) {
    self.navigation.close_panel(self.route(View::Steps, None));
  }
  /// The published version the route views, if it is this page's.
  pub fn viewed(&self) -> Option<VersionNumber> {
    match self.navigation.route_now() {
      Route::Project(project, ProjectPage::PipelineVersion(pipeline, version, _))
        if project == self.project && pipeline == self.pipeline =>
      {
        Some(version)
      }
      _ => None,
    }
  }
  /// Whether `route` is this page's, in any place: moving between them leaves no edits behind.
  pub fn holds(&self, route: &Route) -> bool {
    matches!(route, Route::Project(project, ProjectPage::Pipeline(pipeline, _) | ProjectPage::PipelineVersion(pipeline, ..))
      if *project == self.project && *pipeline == self.pipeline)
  }
  /// The route of `view` of the draft or, with `version`, of that published version.
  pub fn route(&self, view: View, version: Option<VersionNumber>) -> Route {
    Route::Project(
      self.project,
      match version {
        Some(version) => ProjectPage::PipelineVersion(self.pipeline, version, view),
        None => ProjectPage::Pipeline(self.pipeline, view),
      },
    )
  }
  pub const fn project(&self) -> ProjectId {
    self.project
  }
}
impl PartialEq for Views {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.pipeline == other.pipeline
  }
}
impl Eq for Views {}
impl fmt::Debug for Views {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Views")
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
  /// What the route shows, as the page last rendered it; changed only through [`Views`].
  pub view: View,
  pub views: Views,
  pub errors: BTreeMap<String, Message>,
  pub inputs: BTreeMap<String, String>,
  /// The published version shown read-only, as the route names it ([`Views::view`]).
  pub viewing: Option<Pipeline>,
  pub viewing_version: Option<VersionNumber>,
  pub epoch: u64,
  pub comparing: bool,
  pub restore_requested: bool,
  pub published_notice: Option<orchester_core::resource::VersionNumber>,
}
impl DevtoolsInspectable for Editing {}
impl Editing {
  pub fn new(opened: OpenedPipeline, (views, view): (Views, View)) -> Self {
    Self {
      edit: PipelineEdit::new(opened.body.clone()),
      name: orchester_core::name::Name::new(&opened.name).expect("stored resource names are valid"),
      opened,
      view,
      views,
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
  /// Whether the draft or the name has errors or edits not saved yet.
  pub fn unsaved(&self) -> bool {
    self.dirty() || !self.errors.is_empty()
  }
  /// Whether a draft exists beside the published version: saved, or edits not saved yet.
  pub fn has_draft(&self) -> bool {
    self.opened.draft_saved_at.is_some() || self.unsaved()
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
  /// Takes the version viewed into the draft; the caller then returns to the draft ([`Views::to_draft`]).
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
