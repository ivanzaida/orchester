//! The list row's line of a draft some steps of which cannot run (`Pipelines · v2` P8 `vujkk`): the one step and what
//! it lacks, or how many; and how many steps cannot run, for the chip next to *draft*. A published pipeline's row
//! speaks of the version tasks use instead ([`super::words`]).

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_workspace::pipelines::ready::{Blocked, StepProblem, step_problems};

use super::{flow::number, model::Entry, undecided::Said};

/// The row's line of a draft some steps of which cannot run: the one step and what it lacks, or how many.
pub(super) fn line(ctx: &Ctx, entry: &Entry) -> Option<Said> {
  if entry.archived.is_some() || !entry.is_draft() {
    return None;
  }
  let steps = step_problems(entry.flow.as_ref()?);
  let text = match steps.as_slice() {
    [] => return None,
    [only] => step_said(ctx, only),
    many => ctx.t_args("pipeline_hub.line.stepsNotReady", [("count", many.len().to_string())]),
  };
  Some(Said {
    text,
    color: PaletteColor::Warning,
  })
}

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

/// How many steps of a draft cannot run, for the chip next to *draft*; none for an archived pipeline.
pub(super) fn steps_not_ready(entry: &Entry) -> usize {
  match (&entry.flow, entry.archived) {
    (Some(read), None) if entry.is_draft() => step_problems(read).len(),
    _ => 0,
  }
}
