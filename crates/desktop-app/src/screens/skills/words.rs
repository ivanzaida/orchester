//! Skills words use the screen's locale area; domain messages use the workspace namespace.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::resource::InstructionScope;
use orchester_workspace::{agents::data::VersionState, skills::editor::EditorData};
pub fn next(state: VersionState) -> u32 {
  state.published().map_or(1, |v| v.get().saturating_add(1))
}
pub fn version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  match state {
    VersionState::Published(v) => ctx.t_args("skills.version.published", [("version", v.get().to_string())]),
    VersionState::Draft { published: None } => ctx.t_args("skills.version.draftOnly", [("version", "1")]),
    VersionState::Draft { published: Some(v) } => ctx.t_args(
      "skills.version.draft",
      [("published", v.get().to_string()), ("version", next(state).to_string())],
    ),
    VersionState::StaleDraft { based_on, published } => ctx.t_args(
      "skills.version.stale",
      [
        ("basedOn", based_on.map_or(0, |v| v.get()).to_string()),
        ("version", published.map_or(0, |v| v.get()).to_string()),
      ],
    ),
  }
}
pub fn scope(ctx: &Ctx, data: &EditorData, scope: &InstructionScope) -> Arc<str> {
  let option = data.scopes.iter().find(|option| &option.scope == scope);
  match scope {
    InstructionScope::Project => ctx.t("skills.scopes.project"),
    InstructionScope::AgentRole { role } => ctx.t_args("skills.scopes.role", [("role", role.as_str())]),
    InstructionScope::Pipeline { .. } => ctx.t_args(
      "skills.scopes.pipeline",
      [(
        "pipeline",
        option
          .and_then(|o| o.pipeline.clone())
          .unwrap_or_else(|| ctx.t("skills.scopes.removed")),
      )],
    ),
    InstructionScope::Step { .. } => ctx.t_args(
      "skills.scopes.step",
      [
        (
          "pipeline",
          option
            .and_then(|o| o.pipeline.clone())
            .unwrap_or_else(|| ctx.t("skills.scopes.removed")),
        ),
        (
          "step",
          option
            .and_then(|o| o.step.clone())
            .unwrap_or_else(|| ctx.t("skills.scopes.removed")),
        ),
      ],
    ),
  }
}
pub fn files(ctx: &Ctx, count: usize) -> Arc<str> {
  match count {
    0 => ctx.t("skills.noFiles"),
    1 => ctx.t("skills.oneFile"),
    _ => ctx.t_args("skills.filesCount", [("count", count.to_string())]),
  }
}

pub fn types(ctx: &Ctx, instructions: usize, skills: usize) -> Arc<str> {
  ctx.t_args(
    "skills.typeTotals",
    [
      (
        "instructions",
        ctx.t_args(
          if instructions == 1 {
            "skills.instructionOne"
          } else {
            "skills.instructionMany"
          },
          [("count", instructions.to_string())],
        ),
      ),
      (
        "skills",
        ctx.t_args(
          if skills == 1 {
            "skills.skillOne"
          } else {
            "skills.skillMany"
          },
          [("count", skills.to_string())],
        ),
      ),
    ],
  )
}

/// Presentation of a reference path; selection and supported-file rules stay in workspace.
pub fn file_type(path: &str) -> &'static str {
  match std::path::Path::new(path)
    .extension()
    .and_then(|e| e.to_str())
    .map(str::to_ascii_lowercase)
    .as_deref()
  {
    Some("md" | "markdown") => "skills.fileMarkdown",
    Some("sh" | "ps1" | "py" | "js" | "ts" | "rb") => "skills.fileScript",
    Some("txt" | "json" | "yaml" | "yml" | "toml") => "skills.fileText",
    _ => "skills.fileOther",
  }
}
