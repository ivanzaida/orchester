//! The list row's line of a draft some steps of which cannot run (`Pipelines · v2` P8 `vujkk`): the one step and what
//! it lacks, or how many; and how many steps cannot run, for the chip next to *draft*. A published pipeline's row
//! speaks of the version tasks use instead ([`super::words`]).

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_workspace::pipelines::{flow::Runs, ready::step_problems};

use super::{flow::number, model::Entry, undecided::Said};

/// The row's line of a draft some steps of which cannot run: the one step and what it lacks, or how many.
pub(super) fn line(ctx: &Ctx, entry: &Entry) -> Option<Said> {
  if entry.archived.is_some() || !entry.is_draft() {
    return None;
  }
  let steps = step_problems(entry.flow.as_ref()?);
  let text = match steps.as_slice() {
    [] => return None,
    [only] => ctx.t_args(
      line_key(&only.runs),
      [
        ("number", number(only.number)),
        ("step", only.name.to_string()),
        ("tool", tool_name(&only.runs).to_owned()),
      ],
    ),
    many => ctx.t_args("pipeline_hub.line.stepsNotReady", [("count", many.len().to_string())]),
  };
  Some(Said {
    text,
    color: PaletteColor::Warning,
  })
}

fn line_key(runs: &Runs) -> &'static str {
  match runs {
    Runs::NoProfile => "pipeline_hub.line.noProfile",
    Runs::NoTool => "pipeline_hub.line.noTool",
    Runs::ProfileNotPublished { .. } => "pipeline_hub.line.profileNotPublished",
    Runs::ToolNotPublished { .. } => "pipeline_hub.line.toolNotPublished",
    Runs::ToolNotRunnable { .. } => "pipeline_hub.line.toolNotRunnable",
    Runs::Profile { .. } | Runs::Tool { .. } => "pipeline_hub.line.stepsNotReady",
  }
}

/// The name of the tool a step cannot run, for the words that say so; empty for any other.
pub(super) fn tool_name(runs: &Runs) -> &str {
  match runs {
    Runs::ToolNotRunnable { name, .. } => name,
    _ => "",
  }
}

/// How many steps of a draft cannot run, for the chip next to *draft*; none for an archived pipeline.
pub(super) fn steps_not_ready(entry: &Entry) -> usize {
  match (&entry.flow, entry.archived) {
    (Some(read), None) if entry.is_draft() => step_problems(read).len(),
    _ => 0,
  }
}
