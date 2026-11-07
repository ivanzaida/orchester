//! What the panel of a team MCP server says in words that depend on its state: the head's line, its second chip, the
//! day a path was set and what stands under the rows (`Chips`, `Note` in `C4` `SpRvd` … `C8` `lSA4x`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  sharing::{Missing, ProgramProblem},
  time::Timestamp,
};
use orchester_workspace::agents::team_mcp::{McpRuns, TeamMcpSheet};

use crate::{
  screens::team_hub::{
    model::{Entry, Listing},
    words,
  },
  ui::chip::BadgeTone,
};

/// *v3 · program · published by dana-k · Oct 30*: what stands after *MCP server* in the head.
pub(super) fn head_line(ctx: &Ctx, now: Timestamp, entry: &Entry, sheet: &TeamMcpSheet) -> Option<String> {
  let version = entry.number?.to_string();
  let form = ctx.t(match sheet.runs {
    McpRuns::Program { .. } => "team_hub.form.program",
    McpRuns::Remote { .. } => "team_hub.form.remote",
  });
  Some(match &sheet.published {
    Some(published) => ctx
      .t_args(
        "team_hub.mcp.headLine",
        [
          ("version", version),
          ("form", form.to_string()),
          ("by", published.by.clone()),
          ("date", date(ctx, now, published.at).to_string()),
        ],
      )
      .to_string(),
    None => ctx
      .t_args(
        "team_hub.mcp.headLineUnpublished",
        [("version", version), ("form", form.to_string())],
      )
      .to_string(),
  })
}

pub(super) use crate::screens::team_hub::words::date;

/// The program's problem here, when the server has one.
pub(super) fn program_problem(missing: &[Missing]) -> Option<&ProgramProblem> {
  missing.iter().find_map(|part| match part {
    Missing::McpProgram { problem, .. } => Some(problem),
    _ => None,
  })
}

/// The panel's readiness chip: the line of the list, but for a program path, which the panel says in the design's
/// words (`Chip · Needs the program path here` in `SpRvd`, `Program missing here` in `h2md5`).
pub(super) fn chip(ctx: &Ctx, listing: &Listing, entry: &Entry, missing: &[Missing]) -> (Arc<str>, BadgeTone) {
  match (missing, program_problem(missing)) {
    ([_], Some(ProgramProblem::NotSet)) => (ctx.t("team_hub.mcp.chip.pathNotSet"), BadgeTone::Warning),
    ([_], Some(ProgramProblem::FileGone(_))) => (ctx.t("team_hub.mcp.chip.pathGone"), BadgeTone::Danger),
    _ => words::chip(ctx, listing, entry),
  }
}
