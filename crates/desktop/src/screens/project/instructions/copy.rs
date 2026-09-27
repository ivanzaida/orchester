//! Wording shared by the instruction screens: versions, what an instruction applies to, the scope
//! tab it belongs to, and the sources of a prompt.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  resource::{AgentTargets, Exclusion, InjectionEvent, Instruction, InstructionScope, VersionNumber},
  run::{ExcludedInstruction, InstructionVersion, PromptPart, PromptSection, PromptSource},
};

use crate::{agents::data::VersionState, instructions::data::ProjectInstructions, navigation::route::InstructionsTab};

pub fn tab_of(scope: &InstructionScope) -> InstructionsTab {
  match scope {
    InstructionScope::Project => InstructionsTab::ProjectRules,
    InstructionScope::AgentRole { .. } => InstructionsTab::AgentRoles,
    InstructionScope::Pipeline { .. } => InstructionsTab::PipelineOverrides,
    InstructionScope::Step { .. } => InstructionsTab::StepOverrides,
  }
}

fn number(version: VersionNumber) -> String {
  version.get().to_string()
}

/// The number the draft gets when it is published.
pub fn draft_number(state: VersionState) -> u32 {
  let based_on = match state {
    VersionState::Draft { published } => published,
    VersionState::StaleDraft { based_on, .. } => based_on,
    VersionState::Published(version) => Some(version),
  };
  based_on.map_or(1, |version| version.get() + 1)
}

/// "v3 draft", "v2".
pub fn version_label(ctx: &Ctx, state: VersionState) -> Arc<str> {
  if state.has_draft() {
    ctx.t_args(
      "instructions.version.draft",
      [("version", draft_number(state).to_string().as_str())],
    )
  } else {
    let published = state.published().map(number).unwrap_or_default();
    ctx.t_args("instructions.version.published", [("version", published.as_str())])
  }
}

/// The kind of rule an instruction of `part` is: "Project rule", "Pipeline override".
pub fn kind(ctx: &Ctx, part: PromptPart) -> Arc<str> {
  ctx.t(match part {
    PromptPart::AgentRole => "instructions.kind.agentRole",
    PromptPart::ProjectRules | PromptPart::Task => "instructions.kind.projectRule",
    PromptPart::Pipeline => "instructions.kind.pipeline",
    PromptPart::Step => "instructions.kind.step",
  })
}

/// Who the instruction reaches: "All agents", "Builder, Reviewer", the role of an agent role
/// instruction, or the pipeline and step of an override.
pub fn applies_to(ctx: &Ctx, instruction: &Instruction, data: &ProjectInstructions) -> String {
  let unknown = || ctx.t("instructions.removed").to_string();
  match instruction.scope() {
    InstructionScope::AgentRole { role } => role.as_str().to_owned(),
    InstructionScope::Pipeline { pipeline } => data
      .pipeline(*pipeline)
      .map_or_else(unknown, |pipeline| pipeline.name.to_string()),
    InstructionScope::Step { pipeline, step } => data.pipeline(*pipeline).map_or_else(unknown, |pipeline| {
      let step = pipeline
        .steps
        .iter()
        .find(|choice| &choice.key == step)
        .map_or_else(unknown, |choice| choice.name.to_string());
      ctx
        .t_args(
          "instructions.pipelineStep",
          [("pipeline", &*pipeline.name), ("step", step.as_str())],
        )
        .to_string()
    }),
    InstructionScope::Project => targets(ctx, instruction.targets(), data),
  }
}

fn targets(ctx: &Ctx, targets: &AgentTargets, data: &ProjectInstructions) -> String {
  match targets {
    AgentTargets::All => ctx.t("instructions.targets.all").to_string(),
    AgentTargets::Roles(roles) => roles.iter().map(|role| role.as_str()).collect::<Vec<_>>().join(", "),
    AgentTargets::Profiles(profiles) => profiles
      .iter()
      .map(|id| {
        data.profiles.iter().find(|profile| profile.id == *id).map_or_else(
          || ctx.t("instructions.removed").to_string(),
          |profile| profile_label(ctx, profile),
        )
      })
      .collect::<Vec<_>>()
      .join(", "),
  }
}

/// A profile as "Apply to" names it: "Builder profile (builder)".
pub fn profile_label(ctx: &Ctx, profile: &crate::instructions::data::ProfileChoice) -> String {
  ctx
    .t_args(
      "instructions.targets.profile",
      [("role", profile.role.as_str()), ("slug", &*profile.slug)],
    )
    .to_string()
}

pub fn event_label(ctx: &Ctx, event: InjectionEvent) -> Arc<str> {
  ctx.t(match event {
    InjectionEvent::Start => "instructions.events.start",
    InjectionEvent::Resume => "instructions.events.resume",
  })
}

/// "v2", "draft v3".
pub fn instruction_version(ctx: &Ctx, version: InstructionVersion) -> Arc<str> {
  match version {
    InstructionVersion::Published { number: version } => ctx.t_args(
      "instructions.version.published",
      [("version", number(version).as_str())],
    ),
    InstructionVersion::Draft { based_on } => {
      let next = based_on.map_or(1, |version| version.get() + 1);
      ctx.t_args("instructions.version.draftOf", [("version", next.to_string().as_str())])
    }
  }
}

/// A source row's title and the line under it (`lCWtB`, `iLg5E` in `SV0BN`).
pub fn source_text(ctx: &Ctx, section: &PromptSection, pipeline: &str) -> (Arc<str>, Arc<str>) {
  match &section.source {
    PromptSource::AgentProfile { version, .. } => (
      ctx.t("instructions.sources.agentRole"),
      ctx.t_args(
        "instructions.sources.named",
        [("name", section.name.as_str()), ("version", number(*version).as_str())],
      ),
    ),
    PromptSource::Instruction { part, version, .. } => (
      section.name.as_str().into(),
      ctx.t_args(
        "instructions.sources.rule",
        [
          ("kind", &*kind(ctx, *part)),
          ("version", &*instruction_version(ctx, *version)),
        ],
      ),
    ),
    PromptSource::Step { version, .. } => (
      ctx.t("instructions.sources.step"),
      ctx.t_args(
        "instructions.sources.stepOf",
        [
          ("step", section.name.as_str()),
          ("pipeline", pipeline),
          ("version", number(*version).as_str()),
        ],
      ),
    ),
    PromptSource::Task { .. } => (
      ctx.t("instructions.sources.task"),
      ctx.t("instructions.sources.taskDetail"),
    ),
  }
}

/// What a source is: "Agent role", "Project rule", "Step instructions", "Task context".
pub fn source_kind(ctx: &Ctx, source: &PromptSource) -> Arc<str> {
  match source {
    PromptSource::AgentProfile { .. } => ctx.t("instructions.sources.agentRole"),
    PromptSource::Instruction { part, .. } => kind(ctx, *part),
    PromptSource::Step { .. } => ctx.t("instructions.sources.step"),
    PromptSource::Task { .. } => ctx.t("instructions.sources.task"),
  }
}

/// The version a source came from, as the resolved prompt labels it ("Draft v3", "v1"); a task
/// has none.
pub fn source_version(ctx: &Ctx, source: &PromptSource) -> Option<Arc<str>> {
  let published = |version: VersionNumber| {
    ctx.t_args(
      "instructions.version.published",
      [("version", number(version).as_str())],
    )
  };
  match source {
    PromptSource::AgentProfile { version, .. } | PromptSource::Step { version, .. } => Some(published(*version)),
    PromptSource::Instruction {
      version: InstructionVersion::Published { number: version },
      ..
    } => Some(published(*version)),
    PromptSource::Instruction {
      version: InstructionVersion::Draft { based_on },
      ..
    } => {
      let next = based_on.map_or(1, |version| version.get() + 1);
      Some(ctx.t_args(
        "instructions.version.draftLabel",
        [("version", next.to_string().as_str())],
      ))
    }
    PromptSource::Task { .. } => None,
  }
}

/// "Excluded: Review checklist v2" and why: the agents it is for, or where or when it applies.
pub fn excluded(ctx: &Ctx, excluded: &ExcludedInstruction, data: &ProjectInstructions) -> Arc<str> {
  let only = |agents: String| ctx.t_args("instructions.excluded.onlyFor", [("agents", agents.as_str())]);
  let reason = match (excluded.reason, excluded.body.scope()) {
    (Exclusion::OtherRole, InstructionScope::AgentRole { role }) => only(role.as_str().to_owned()),
    (Exclusion::OtherAgents, _) => only(targets(ctx, excluded.body.targets(), data)),
    (Exclusion::OtherRole, _) => ctx.t("instructions.excluded.otherRole"),
    (Exclusion::OtherPipeline, _) => ctx.t("instructions.excluded.otherPipeline"),
    (Exclusion::OtherStep, _) => ctx.t("instructions.excluded.otherStep"),
    (Exclusion::OtherEvent, _) => ctx.t("instructions.excluded.otherEvent"),
  };
  ctx.t_args(
    "instructions.preview.excluded",
    [
      ("name", excluded.name.as_str()),
      ("version", &*instruction_version(ctx, excluded.version)),
      ("reason", &*reason),
    ],
  )
}
