//! Presentation state for Instructions fePhH; instruction validation stays in workspace.
use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{id::ResourceId, resource::InstructionScope};
use orchester_workspace::instructions::data::InstructionEntry;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
  #[default]
  Project,
  Roles,
  Pipelines,
  Steps,
}
impl DevtoolsInspectable for Tab {}
impl Tab {
  pub const ALL: [Self; 4] = [Self::Project, Self::Roles, Self::Pipelines, Self::Steps];
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
  Rule(Tab, Option<ResourceId>),
  New(Tab),
  Import,
}
impl DevtoolsInspectable for View {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved(pub Option<InstructionEntry>);
impl DevtoolsInspectable for Saved {}
#[derive(Clone)]
pub struct Selection {
  pub view: Signal<View>,
  pub pending: Signal<Option<View>>,
  pub created: Signal<Saved>,
  pub dirty: Signal<bool>,
  pub busy: Signal<bool>,
  pub full: Signal<bool>,
}
impl Selection {
  pub fn open(&self, view: View) {
    if self.dirty.get_untracked() || self.busy.get_untracked() {
      self.pending.set(Some(view));
    } else {
      self.full.set(false);
      self.view.set(view);
    }
  }
}
