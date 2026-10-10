//! UI state; graph edits and their validation remain the core's.
use std::collections::BTreeMap;

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::resource::{Pipeline, PipelineEdit, Step, StepKey};
use orchester_workspace::{messages::Message, pipelines::data::OpenedPipeline};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
  Pipeline,
  Step(StepKey),
  Settings,
  History,
  Publish,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
  pub opened: OpenedPipeline,
  pub name: orchester_core::name::Name,
  pub edit: PipelineEdit,
  pub panel: Panel,
  pub errors: BTreeMap<String, Message>,
  pub inputs: BTreeMap<String, String>,
  pub viewing: Option<Pipeline>,
  pub viewing_version: Option<orchester_core::resource::VersionNumber>,
  pub epoch: u64,
  pub comparing: bool,
  pub published_notice: Option<orchester_core::resource::VersionNumber>,
}
impl DevtoolsInspectable for Editing {}
impl Editing {
  pub fn new(opened: OpenedPipeline) -> Self {
    Self {
      edit: PipelineEdit::new(opened.body.clone()),
      name: orchester_core::name::Name::new(&opened.name).expect("stored resource names are valid"),
      opened,
      panel: Panel::Pipeline,
      errors: BTreeMap::new(),
      inputs: BTreeMap::new(),
      viewing: None,
      viewing_version: None,
      epoch: 0,
      comparing: false,
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
  pub fn saved(&mut self, opened: OpenedPipeline) {
    let latest = opened.latest.as_ref().map(|v| v.number);
    if latest != self.opened.latest.as_ref().map(|v| v.number) {
      self.published_notice = latest;
    }
    self.opened = opened;
  }
}
pub type State = Signal<Editing>;
