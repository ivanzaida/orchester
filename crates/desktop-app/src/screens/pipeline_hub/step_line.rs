//! What a row's line and *Ready?* say of a step that cannot run (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): the one
//! step and what it lacks.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::pipelines::ready::{Blocked, StepProblem};

use super::flow::number;
/// The line of one step that cannot run: *02 Review has no agent profile*.
pub(super) fn step_said(ctx: &Ctx, step: &StepProblem) -> Arc<str> {
  ctx.t_args(
    line_key(&step.blocked),
    [
      ("number", number(step.number)),
      ("step", step.name.to_string()),
      ("tool", named(&step.blocked).to_owned()),
      ("profile", named(&step.blocked).to_owned()),
    ],
  )
}

const fn line_key(blocked: &Blocked) -> &'static str {
  match blocked {
    Blocked::NoProfile => "pipeline_hub.line.noProfile",
    Blocked::NoTool => "pipeline_hub.line.noTool",
    Blocked::ProfileNotPublished { .. } => "pipeline_hub.line.profileNotPublished",
    Blocked::Gone { tool: false, .. } => "pipeline_hub.line.profileGone",
    Blocked::Gone { tool: true, .. } => "pipeline_hub.line.toolGone",
    Blocked::ProfileConnectionGone { .. } => "pipeline_hub.line.profileConnectionGone",
    Blocked::ToolNotPublished { .. } => "pipeline_hub.line.toolNotPublished",
    Blocked::ToolNotRunnable { .. } => "pipeline_hub.line.toolNotRunnable",
  }
}

/// The name of the profile or tool a step names that cannot run, for the words that say so; empty for any other.
pub(super) fn named(blocked: &Blocked) -> &str {
  match blocked {
    Blocked::ToolNotRunnable { name, .. } | Blocked::ProfileConnectionGone { name, .. } => name,
    Blocked::NoProfile
    | Blocked::NoTool
    | Blocked::ProfileNotPublished { .. }
    | Blocked::Gone { .. }
    | Blocked::ToolNotPublished { .. } => "",
  }
}
