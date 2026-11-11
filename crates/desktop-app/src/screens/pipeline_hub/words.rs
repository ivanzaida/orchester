//! What a row and a panel say of a pipeline (`Pipelines · v2` P1, P8): how many steps and tasks, or what the person has
//! to do about it, and its version.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{resource::ResourceKind, time::Timestamp};
use orchester_workspace::{
  pipelines::{
    flow::FlowRead,
    ready::{Basis, Problem, problems},
  },
  readiness::{ReadinessStanding, Undecided},
};

use super::{
  model::Entry,
  ready::whole_words,
  step_line,
  undecided::{Said, undecided_words},
  version_view::Viewed,
};
use crate::screens::{
  local_hub::words::{archived_at, missing, undecided},
  team_hub::words::date,
};

/// The line under a pipeline's name: when it was archived, what keeps it from running (in `warning`), or the steps and
/// tasks; with `unsaved` (its panel holds changes) that they are not saved, in front of the rest.
pub(super) fn line(ctx: &Ctx, now: Timestamp, entry: &Entry, unsaved: bool) -> Said {
  let said = state(ctx, now, entry);
  if !unsaved {
    return said;
  }
  Said {
    text: ctx.t_args("pipeline_hub.line.unsaved", [("rest", said.text.to_string())]),
    color: PaletteColor::Warning,
  }
}

fn state(ctx: &Ctx, now: Timestamp, entry: &Entry) -> Said {
  if let Some(at) = entry.archived {
    return Said {
      text: archived_at(ctx, now, at),
      color: PaletteColor::TextMuted,
    };
  }
  if let Some(text) = needs(ctx, entry) {
    return Said {
      text,
      color: PaletteColor::Warning,
    };
  }
  Said {
    text: summary(ctx, entry),
    color: PaletteColor::TextSecondary,
  }
}

/// What the person has to do before the pipeline can run: what its profiles and tools lack; `None` for a pipeline that
/// is ready. A published pipeline says what *Ready?* lists of its published version ([`told`]).
pub(super) fn needs(ctx: &Ctx, entry: &Entry) -> Option<Arc<str>> {
  if let Some(read) = &entry.published {
    return told(ctx, entry, read);
  }
  match entry.standing.as_ref()? {
    ReadinessStanding::Ready => None,
    ReadinessStanding::Missing(readiness) => Some(match readiness.missing() {
      [only] => missing(ctx, ResourceKind::Pipeline, only),
      many => ctx.t_args("local_hub.line.many", [("count", many.len().to_string())]),
    }),
    ReadinessStanding::Undecided(why) => Some(undecided_words(ctx, why, &entry.undecided)),
  }
}

/// What *Ready?* lists of a published pipeline, as the row and the chip say it: the one problem, or how many (*2 steps
/// not ready* when they all are steps, else *3 things missing*), so the three never tell different stories.
fn told(ctx: &Ctx, entry: &Entry, read: &FlowRead) -> Option<Arc<str>> {
  let found = problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    runs: None,
  });
  Some(match found.as_slice() {
    [] => return None,
    [only] => one(ctx, entry, only),
    many if many.iter().all(|problem| matches!(problem, Problem::Step(_))) => {
      ctx.t_args("pipeline_hub.line.stepsNotReady", [("count", many.len().to_string())])
    }
    many => ctx.t_args("local_hub.line.many", [("count", many.len().to_string())]),
  })
}

/// One problem *Ready?* lists, as a row's line says it.
fn one(ctx: &Ctx, entry: &Entry, problem: &Problem) -> Arc<str> {
  match problem {
    Problem::Step(step) => step_line::step_said(ctx, step),
    Problem::Member(member) => match &member.why {
      Undecided::ConnectionGone(_) => {
        ctx.t_args("pipeline_hub.line.connectionGone", [("name", member.name.to_string())])
      }
      other => undecided(ctx, other),
    },
    Problem::Missing(part) => missing(ctx, ResourceKind::Pipeline, part),
    Problem::Undecided(why) => undecided_words(ctx, why, &entry.undecided),
    Problem::Whole(whole) => whole_words(ctx, *whole),
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

/// The version at a row's end: *v3*; none for an archived pipeline that never had one.
pub(super) fn version(entry: &Entry) -> Option<String> {
  entry.number.map(|number| number.to_string())
}

/// What stands after *Pipeline* in a panel's head: *v3 · published Oct 27*, *v3 · archived Nov 6*; none for an
/// archived pipeline that never had a version.
pub(super) fn head_version(ctx: &Ctx, now: Timestamp, entry: &Entry) -> Option<String> {
  let number = entry.number?;
  let (key, at) = match (entry.archived, entry.published_at) {
    (Some(at), _) => ("pipeline_hub.panel.archivedAt", at),
    (None, Some(at)) => ("pipeline_hub.panel.published", at),
    (None, None) => return Some(number.to_string()),
  };
  Some(
    ctx
      .t_args(
        key,
        [
          ("version", number.to_string()),
          ("date", date(ctx, now, at).to_string()),
        ],
      )
      .to_string(),
  )
}

/// What stands after *Pipeline* in the head while the panel shows a version read-only: *Viewing v2 · published Nov 3*,
/// so the head never names the latest version for a body that is an older one.
pub(super) fn head_viewing(ctx: &Ctx, now: Timestamp, viewed: &Viewed) -> String {
  ctx
    .t_args(
      "pipeline_hub.panel.viewing",
      [
        ("version", viewed.number.to_string()),
        ("date", date(ctx, now, viewed.published_at).to_string()),
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
