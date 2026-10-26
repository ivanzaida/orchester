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
pub fn scoped_event(ctx: &Ctx, events: Events, role: bool) -> Arc<str> {
  if role && events == Events::StartAndResume {
    ctx.t("instructions.roleStartResume")
  } else {
    event(ctx, events)
  }
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
/// What the draft bar says of a draft automation saved over Orchester's MCP server (ORC-30).
pub fn automation_version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  match state {
    VersionState::Draft { published: Some(v) } => {
      ctx.t_args("instructions.automationDraftSaved", [("version", v.get().to_string())])
    }
    VersionState::Draft { published: None } => ctx.t("instructions.automationNewDraft"),
    _ => version(ctx, state),
  }
}
pub fn footer(ctx: &Ctx, tab: Tab, version: Option<orchester_core::resource::VersionNumber>) -> Arc<str> {
  version.map_or_else(
    || ctx.t("instructions.newPublishNote"),
    |version| {
      ctx.t_args(
        if matches!(tab, Tab::Roles | Tab::Steps) {
          "instructions.editingNote"
        } else {
          "instructions.publishNote"
        },
        [("version", version.get().to_string())],
      )
    },
  )
}
pub fn published_notice(
  ctx: &Ctx,
  version: u32,
  retained: &[orchester_workspace::instructions::publication::RetainedRun],
) -> Arc<str> {
  if retained.is_empty() {
    return ctx.t_args("instructions.publishedNotice", [("version", version.to_string())]);
  }
  let runs = retained
    .iter()
    .map(|run| {
      ctx
        .t_args(
          "instructions.retainedRun",
          [
            ("task", run.task.clone()),
            ("number", run.number.get().to_string()),
            ("version", run.version.to_string()),
          ],
        )
        .to_string()
    })
    .collect::<Vec<_>>()
    .join(", ");
  ctx.t_args(
    if retained.len() == 1 {
      "instructions.publishedNoticeOne"
    } else {
      "instructions.publishedNoticeMany"
    },
    [
      ("version", version.to_string()),
      ("count", retained.len().to_string()),
      ("runs", runs),
    ],
  )
}

pub fn publication_failure(
  ctx: &Ctx,
  published: Option<orchester_core::resource::VersionNumber>,
  error: &orchester_workspace::messages::Message,
) -> (Arc<str>, Arc<str>) {
  let draft = published.map_or(1, |version| version.get().saturating_add(1));
  let title = ctx.t_args("instructions.publishFailed", [("version", draft.to_string())]);
  let detail = crate::ui::message::message_text(ctx, error);
  let body = if let Some(version) = published {
    ctx.t_args(
      "instructions.publishFailedRetained",
      [
        ("reason", detail.to_string()),
        ("draft", draft.to_string()),
        ("published", version.get().to_string()),
      ],
    )
  } else {
    ctx.t_args(
      "instructions.publishFailedUnpublished",
      [("reason", detail.to_string()), ("draft", draft.to_string())],
    )
  };
  (title, body)
}
