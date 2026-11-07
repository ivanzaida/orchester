//! What the panel of a team MCP server says in words that depend on its state: the head's line, its second chip, the
//! day a path was set and what stands under the rows (`Chips`, `Note` in `C4` `SpRvd` … `C8` `lSA4x`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  secret::SecretName,
  sharing::{Missing, ProgramProblem},
  time::Timestamp,
};
use orchester_workspace::{
  agents::team_mcp::{McpRuns, TeamMcpSheet},
  display,
};

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

/// The day `at` as the panel says it, on the day `now`: *today*, the month and day in this year, or with the year
/// after them.
pub(super) fn date(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
  let (month, day, year) = display::calendar_date(at);
  let now = display::calendar_date(now);
  if (month, day, year) == now {
    return ctx.t("team_hub.mcp.date.today");
  }
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  if year == now.2 {
    return ctx.t_args(
      "team_hub.mcp.date.thisYear",
      [("month", month.to_string()), ("day", day.to_string())],
    );
  }
  ctx.t_args(
    "team_hub.mcp.date.otherYear",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("year", year.to_string()),
    ],
  )
}

/// The program's problem here, when the server has one.
pub(super) fn program_problem(missing: &[Missing]) -> Option<&ProgramProblem> {
  missing.iter().find_map(|part| match part {
    Missing::McpProgram { problem, .. } => Some(problem),
    _ => None,
  })
}

/// How a secret's value stands here.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Value {
  Set,
  Missing,
  /// The keychain could not say.
  Unreadable,
}

pub(super) fn value_of(missing: &[Missing], name: &SecretName) -> Value {
  missing
    .iter()
    .find_map(|part| match part {
      Missing::Secret { name: lacking } if lacking == name => Some(Value::Missing),
      Missing::SecretUnreadable { name: unsure } if unsure == name => Some(Value::Unreadable),
      _ => None,
    })
    .unwrap_or(Value::Set)
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
