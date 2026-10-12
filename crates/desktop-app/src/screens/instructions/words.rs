//! Localized scope, timing and lifecycle labels.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  resource::{AgentTargets, Exclusion},
  run::{InstructionVersion, PromptPart},
};
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{data::ProjectInstructions, form::Events},
};

use super::state::Tab;

pub fn version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  let number = state.published().map_or(1, |v| v.get().saturating_add(1));
  match state {
    VersionState::Published(v) => ctx.t_args("instructions.published", [("version", v.get().to_string())]),
    VersionState::Draft { published: Some(v) } => ctx.t_args(
      "instructions.draftSaved",
      [("published", v.get().to_string()), ("version", number.to_string())],
    ),
    VersionState::Draft { published: None } => ctx.t("instructions.newDraft"),
    VersionState::StaleDraft { .. } => ctx.t("instructions.stale"),
  }
}
pub fn event(ctx: &Ctx, events: Events) -> Arc<str> {
  ctx.t(match events {
    Events::Start => "instructions.start",
    Events::Resume => "instructions.resume",
    Events::StartAndResume => "instructions.startResume",
  })
}
pub fn scope(ctx: &Ctx, tab: Tab) -> Arc<str> {
  ctx.t(match tab {
    Tab::Project => "instructions.projectRule",
    Tab::Roles => "instructions.roleRule",
    Tab::Pipelines => "instructions.pipelineRule",
    Tab::Steps => "instructions.stepRule",
  })
}
pub fn targets(ctx: &Ctx, targets: &AgentTargets, data: &ProjectInstructions) -> Arc<str> {
  match targets {
    AgentTargets::All => ctx.t("instructions.all"),
    AgentTargets::Roles(roles) => roles.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(", ").into(),
    AgentTargets::Profiles(ids) => ids
      .iter()
      .map(|id| {
        data
          .profiles
          .iter()
          .find(|p| p.id == *id)
          .map_or_else(|| id.to_string(), |p| p.role.as_str().to_owned())
      })
      .collect::<Vec<_>>()
      .join(", ")
      .into(),
  }
}
pub fn part(ctx: &Ctx, part: PromptPart) -> Arc<str> {
  ctx.t(match part {
    PromptPart::AgentRole => "instructions.sourceRole",
    PromptPart::ProjectRules => "instructions.projectRule",
    PromptPart::Pipeline => "instructions.pipelineRule",
    PromptPart::Step => "instructions.stepRule",
    PromptPart::Task => "instructions.sourceTask",
  })
}
pub fn preview_version(ctx: &Ctx, version: InstructionVersion) -> Arc<str> {
  match version {
    InstructionVersion::Published { number } => {
      ctx.t_args("instructions.version", [("version", number.get().to_string())])
    }
    InstructionVersion::Draft { based_on } => ctx.t_args(
      "instructions.draftVersion",
      [("version", based_on.map_or(1, |v| v.get().saturating_add(1)).to_string())],
    ),
  }
}
pub fn excluded(ctx: &Ctx, reason: Exclusion) -> Arc<str> {
  ctx.t(match reason {
    Exclusion::OtherRole => "instructions.excludedRole",
    Exclusion::OtherAgents => "instructions.excludedAgents",
    Exclusion::OtherPipeline => "instructions.excludedPipeline",
    Exclusion::OtherStep => "instructions.excludedStep",
    Exclusion::OtherEvent => "instructions.excludedEvent",
  })
}
