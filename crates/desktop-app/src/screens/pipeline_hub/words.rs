//! What a row and a panel say of a pipeline (`Pipelines · v2` P1, P8): how many steps and tasks, or what the person has
//! to do about it, and its version.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{resource::ResourceKind, time::Timestamp};
use orchester_workspace::readiness::ReadinessStanding;

use super::model::Entry;
use crate::{
  screens::{
    local_hub::words::{archived_at, missing, undecided},
    team_hub::words::date,
  },
  ui::message::message_text,
};

/// What a row says and the color it says it in.
pub(super) struct Said {
  pub text: Arc<str>,
  pub color: PaletteColor,
}

/// The line under a pipeline's name: when it was archived, what keeps it from running, or its steps and tasks.
pub(super) fn line(ctx: &Ctx, now: Timestamp, entry: &Entry) -> Said {
  if let Some(at) = entry.archived {
    return Said {
      text: archived_at(ctx, now, at),
      color: PaletteColor::TextMuted,
    };
  }
  match needs(ctx, entry) {
    Some(text) => Said {
      text,
      color: PaletteColor::Warning,
    },
    None => Said {
      text: summary(ctx, entry),
      color: PaletteColor::TextSecondary,
    },
  }
}

/// What the person has to do before the pipeline can run: its draft's first problem or that it is not published, or
/// what its profiles and tools lack; `None` for a pipeline that is ready.
pub(super) fn needs(ctx: &Ctx, entry: &Entry) -> Option<Arc<str>> {
  if entry.is_draft() {
    return Some(entry.problem.as_ref().map_or_else(
      || ctx.t("pipeline_hub.line.notPublished"),
      |problem| message_text(ctx, problem),
    ));
  }
  match entry.standing.as_ref()? {
    ReadinessStanding::Ready => None,
    ReadinessStanding::Missing(readiness) => Some(match readiness.missing() {
      [only] => missing(ctx, ResourceKind::Pipeline, only),
      many => ctx.t_args("local_hub.line.many", [("count", many.len().to_string())]),
    }),
    ReadinessStanding::Undecided(why) => Some(undecided(ctx, why)),
  }
}

/// *6 steps · used by 13 tasks*.
fn summary(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  let key = match (entry.step_count, entry.used_by) {
    (1, 0) => "pipeline_hub.line.oneStepNoTask",
    (1, 1) => "pipeline_hub.line.oneStepOneTask",
    (1, _) => "pipeline_hub.line.oneStepTasks",
    (_, 0) => "pipeline_hub.line.stepsNoTask",
    (_, 1) => "pipeline_hub.line.stepsOneTask",
    (..) => "pipeline_hub.line.stepsTasks",
  };
  ctx.t_args(
    key,
    [
      ("steps", entry.step_count.to_string()),
      ("tasks", entry.used_by.to_string()),
    ],
  )
}

/// The version at a row's end: *v3*, or *draft* while there is none.
pub(super) fn version(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  entry.number.map_or_else(
    || ctx.t("pipeline_hub.row.draft"),
    |number| Arc::from(number.to_string()),
  )
}

/// What stands after *Pipeline* in a panel's head: *v3 · published Oct 27*, *v3 · archived Nov 6*, or that it is a
/// draft.
pub(super) fn head_version(ctx: &Ctx, now: Timestamp, entry: &Entry) -> String {
  let Some(number) = entry.number else {
    return ctx.t("pipeline_hub.panel.draft").to_string();
  };
  let (key, at) = match (entry.archived, entry.published_at) {
    (Some(at), _) => ("pipeline_hub.panel.archivedAt", at),
    (None, Some(at)) => ("pipeline_hub.panel.published", at),
    (None, None) => return number.to_string(),
  };
  ctx
    .t_args(
      key,
      [
        ("version", number.to_string()),
        ("date", date(ctx, now, at).to_string()),
      ],
    )
    .to_string()
}

/// *used by 13 tasks*, at the end of the chips.
pub(super) fn used_by(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  match entry.used_by {
    0 => ctx.t("pipeline_hub.panel.usedNone"),
    1 => ctx.t("pipeline_hub.panel.usedOne"),
    count => ctx.t_args("pipeline_hub.panel.used", [("count", count.to_string())]),
  }
}
