//! Wording shared by the skill screens: a skill's version and scope as the library, the list and
//! the editor name them.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::resource::InstructionScope;

use super::super::instructions::copy::draft_number;
use crate::{agents::data::VersionState, skills::data::ProjectSkills};

fn number(version: Option<orchester_core::resource::VersionNumber>) -> String {
  version.map(|version| version.get().to_string()).unwrap_or_default()
}

/// The library's Version column (`gzZlq` in `hn8qO`): "Published v2", "v1 · Draft v2 saved",
/// "Draft v1 · not published".
pub fn version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  let draft = draft_number(state).to_string();
  match state {
    VersionState::Published(version) => ctx.t_args(
      "skills.version.published",
      [("version", number(Some(version)).as_str())],
    ),
    VersionState::Draft { published: None } => ctx.t_args("skills.version.draftOnly", [("version", draft.as_str())]),
    VersionState::Draft { published } => ctx.t_args(
      "skills.version.draftSaved",
      [("published", number(published).as_str()), ("version", draft.as_str())],
    ),
    VersionState::StaleDraft { based_on, published } => ctx.t_args(
      "skills.version.stale",
      [
        ("basedOn", number(based_on).as_str()),
        ("version", number(published).as_str()),
      ],
    ),
  }
}

/// The list's short version (`S9Jfg` in `jNTVN`): "v3 · Draft v4", "v2", "Draft v1".
pub fn short_version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  let draft = draft_number(state).to_string();
  match (state.published(), state.has_draft()) {
    (Some(published), false) => ctx.t_args(
      "skills.version.short.published",
      [("version", number(Some(published)).as_str())],
    ),
    (Some(published), true) => ctx.t_args(
      "skills.version.short.draftOf",
      [
        ("published", number(Some(published)).as_str()),
        ("version", draft.as_str()),
      ],
    ),
    (None, _) => ctx.t_args("skills.version.short.draft", [("version", draft.as_str())]),
  }
}

/// Where a skill applies, in words: "Project", "Role · Reviewer", "Pipeline · Feature delivery",
/// "Step · Run checks"; with `pipeline_of_step`, a step names its pipeline too (the editor's select).
pub fn scope(ctx: &Ctx, scope: &InstructionScope, data: &ProjectSkills, pipeline_of_step: bool) -> Arc<str> {
  let removed = || ctx.t("skills.scope.removed");
  match scope {
    InstructionScope::Project => ctx.t("skills.scope.project"),
    InstructionScope::AgentRole { role } => ctx.t_args("skills.scope.role", [("role", role.as_str())]),
    InstructionScope::Pipeline { pipeline } => {
      let name = data
        .pipeline(*pipeline)
        .map_or_else(removed, |pipeline| pipeline.name.clone());
      ctx.t_args("skills.scope.pipeline", [("pipeline", &*name)])
    }
    InstructionScope::Step { pipeline, step } => {
      let pipeline = data.pipeline(*pipeline);
      let step = pipeline
        .and_then(|pipeline| pipeline.steps.iter().find(|choice| &choice.key == step))
        .map_or_else(removed, |choice| choice.name.clone());
      match (pipeline_of_step, pipeline) {
        (true, Some(pipeline)) => ctx.t_args(
          "skills.scope.pipelineStep",
          [("pipeline", &*pipeline.name), ("step", &*step)],
        ),
        _ => ctx.t_args("skills.scope.step", [("step", &*step)]),
      }
    }
  }
}

/// What the scope names, short, for the list: "Project", the role, the pipeline or the step.
pub fn scope_short(ctx: &Ctx, scope: &InstructionScope, data: &ProjectSkills) -> Arc<str> {
  let removed = || ctx.t("skills.scope.removed");
  match scope {
    InstructionScope::Project => ctx.t("skills.scope.project"),
    InstructionScope::AgentRole { role } => role.as_str().into(),
    InstructionScope::Pipeline { pipeline } => data
      .pipeline(*pipeline)
      .map_or_else(removed, |pipeline| pipeline.name.clone()),
    InstructionScope::Step { pipeline, step } => data
      .pipeline(*pipeline)
      .and_then(|pipeline| pipeline.steps.iter().find(|choice| &choice.key == step))
      .map_or_else(removed, |choice| choice.name.clone()),
  }
}

/// The kind of skill its scope makes it, for the status label: "Project", "Role", "Pipeline",
/// "Step".
pub fn kind(ctx: &Ctx, scope: &InstructionScope) -> Arc<str> {
  ctx.t(match scope {
    InstructionScope::Project => "skills.kind.project",
    InstructionScope::AgentRole { .. } => "skills.kind.role",
    InstructionScope::Pipeline { .. } => "skills.kind.pipeline",
    InstructionScope::Step { .. } => "skills.kind.step",
  })
}
