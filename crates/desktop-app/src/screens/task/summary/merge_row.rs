//! The *Merge* row of a task's properties (`Merge` `k81Vk` in 01h `fUHdZ`; its states in `Cjprb` › *Task page ·
//! Properties › Merge row*): under Branch once the latest run waits for review or its merge was attempted, absent
//! before. It says how the run's last merge attempt left it, as the run's attempt page tells it (ORC-363,
//! [`Standing`]): *Waits for your review* and *Retrying · N of 3* in `info`, *Not merged*, *Git stopped*, *Files left
//! changed*, *Conflicts · N files* and *Not recorded* in `warning`, *Merged* in `success`, which stays after the merge.
//! The whole row opens that page, with the open arrow at its end; the Summary's top card says the same outcome
//! (`crate::screens::task::merge`, 01j `k4C0Es`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  integration::{
    attempt::{AttemptOutcome, MergeAsker},
    retry::RETRIES,
  },
  time::Timestamp,
};
use orchester_workspace::tasks::{detail::RunView, merge_attempts::page::AttemptPage, needs_you::WaitingKind};

use super::{properties::row, runs::line::day_and_clock};
use crate::{
  navigation::Navigation,
  project::TasksState,
  routes::{ProjectPage, Route},
  screens::task::{
    TaskContext,
    merge::attempt::{
      read::{Read, read},
      standing::Standing,
    },
  },
  theme::{ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The Merge row of `task`'s latest run, read on its own; `None` while the run still works, as it has no merge to tell
/// yet.
pub(super) fn merge_row(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let run = task.detail.run.as_ref()?;
  if run.run.status().has_live_work() && task.detail.merge.is_none() {
    return None;
  }
  Some(ctx.mount_keyed::<MergeRow>(&format!("task-merge-row-{}", run.id()), task.clone()))
}

/// What the row says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
  Waits,
  NotMerged(Timestamp),
  /// The attempt Orchester tries next, of [`RETRIES`].
  Retrying(u32),
  GitStopped(Timestamp),
  LeftChanged,
  Conflicts(usize),
  NotRecorded(Timestamp),
  Merged(Timestamp),
  /// Merged by git outside Orchester, which recorded no time for it.
  MergedOutside,
}

impl Shown {
  /// How `page`'s run stands; `waits` when no merge was tried and the run waits for the person's review.
  fn of(page: &AttemptPage, waits: bool) -> Option<Self> {
    let Some(standing) = Standing::of(page) else {
      return waits.then_some(Self::Waits);
    };
    let last = page.last().map(|recorded| &recorded.attempt);
    let at = last.map_or_else(Timestamp::now, |attempt| attempt.at());
    Some(match standing {
      Standing::Merged => Self::Merged(page.merge.as_ref().map_or(at, |merge| merge.merged_at)),
      Standing::MergedOutside => Self::MergedOutside,
      Standing::Retrying(_) => Self::Retrying(match last.map(|attempt| attempt.asked_by()) {
        Some(MergeAsker::Retry { number }) => number.get().saturating_add(1).min(RETRIES),
        _ => 1,
      }),
      Standing::Conflicts => Self::Conflicts(match last.map(|attempt| attempt.outcome()) {
        Some(AttemptOutcome::Conflicts { files }) => files.len(),
        _ => 0,
      }),
      Standing::NotRecorded => Self::NotRecorded(at),
      Standing::LeftChanged => Self::LeftChanged,
      Standing::GitStopped => Self::GitStopped(at),
      Standing::AwaitingApproval | Standing::Resolved => Self::Waits,
      Standing::QueueStopped | Standing::Interrupted | Standing::Moved | Standing::NotMerged => Self::NotMerged(at),
    })
  }

  /// What the task's own detail tells before the run's attempts are read, or when they cannot be: only a merge
  /// recorded, as a run that waits for review may have attempts that did not land.
  fn known(task: &TaskContext) -> Option<Self> {
    task.detail.merge.as_ref().map(|merge| Self::Merged(merge.merged_at))
  }

  fn glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Waits => (Icon::Eye, PaletteColor::Info),
      Self::NotMerged(_) => Standing::NotMerged.glyph(),
      Self::Retrying(_) => (Icon::RefreshCw, PaletteColor::Info),
      Self::GitStopped(_) => Standing::GitStopped.glyph(),
      Self::LeftChanged => Standing::LeftChanged.glyph(),
      Self::Conflicts(_) => Standing::Conflicts.glyph(),
      Self::NotRecorded(_) => Standing::NotRecorded.glyph(),
      Self::Merged(_) => Standing::Merged.glyph(),
      Self::MergedOutside => Standing::MergedOutside.glyph(),
    }
  }

  /// The name of the state in the row's `state` description.
  const fn name(self) -> &'static str {
    match self {
      Self::Waits => "waits",
      Self::NotMerged(_) => "not_merged",
      Self::Retrying(_) => "retrying",
      Self::GitStopped(_) => "git_stopped",
      Self::LeftChanged => "left_changed",
      Self::Conflicts(_) => "conflicts",
      Self::NotRecorded(_) => "not_recorded",
      Self::Merged(_) => "merged",
      Self::MergedOutside => "merged_outside",
    }
  }

  fn words(self, ctx: &Ctx) -> Arc<str> {
    let timed = |key: &str, at: Timestamp| ctx.t_args(key, [("time", day_and_clock(ctx, at))]);
    match self {
      Self::Waits => ctx.t("summary.properties.merge.waits"),
      Self::NotMerged(at) => timed("summary.properties.merge.notMerged", at),
      Self::Retrying(attempt) => ctx.t_args(
        "summary.properties.merge.retrying",
        [("attempt", attempt.to_string()), ("of", RETRIES.to_string())],
      ),
      Self::GitStopped(at) => timed("summary.properties.merge.gitStopped", at),
      Self::LeftChanged => ctx.t("summary.properties.merge.leftChanged"),
      Self::Conflicts(1) => ctx.t("summary.properties.merge.conflictsOne"),
      Self::Conflicts(count) => ctx.t_args(
        "summary.properties.merge.conflictsOther",
        [("count", count.to_string())],
      ),
      Self::NotRecorded(at) => timed("summary.properties.merge.notRecorded", at),
      Self::Merged(at) => timed("summary.properties.merge.merged", at),
      Self::MergedOutside => ctx.t("summary.properties.merge.mergedOutside"),
    }
  }
}

/// Whether `run` waits for the person's review before it merges, on its task page or in the merge queue, as the
/// project's tasks were last read.
fn waits_for_review(task: &TaskContext, run: &RunView) -> bool {
  match task.project.feed.tasks.get() {
    TasksState::Ready(tasks) => tasks
      .waiting()
      .iter()
      .any(|item| item.kind == WaitingKind::ReadyToMerge && item.run.is_some_and(|(waiting, _)| waiting == run.id())),
    TasksState::Loading | TasksState::Failed(_) => false,
  }
}

/// Reads the run's attempts again whenever the project's tasks or runs change, the merge queue moves or a merge job
/// starts or ends, as its attempt page does.
struct MergeRow;

impl Component for MergeRow {
  type Props = TaskContext;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let task = ctx.props::<TaskContext>().clone();
    let Some(run) = task.detail.run.as_ref() else {
      return Element::from(Column::new());
    };
    let number = run.run.number();
    let waits = waits_for_review(&task, run);
    let mut in_review = false;
    let shown = match read(ctx, &task.project, (&task.key, number), 0) {
      Read::Ready(page) => {
        // A run whose merge was tried is no longer in review: its attempts tell where it stands.
        in_review = waits && Standing::of(&page).is_none();
        Shown::of(&page, waits)
      }
      Read::Reading | Read::Missing => Shown::known(&task),
      // Why is said on the run's attempt page, which the Merge row opens.
      Read::Failed(message) => {
        tracing::warn!(?message, run = %run.id(), "cannot read the run's merge attempts for its Merge row");
        Shown::known(&task)
      }
    };
    let Some(shown) = shown else {
      return Column::new().into();
    };
    let page = if in_review {
      ProjectPage::TaskReview(task.key.clone())
    } else {
      ProjectPage::TaskMerge(task.key.clone(), number)
    };
    let route = Route::Project(task.project.open.project.id(), page);
    let open = Navigation::of(ctx).to(route);
    let words = shown.words(ctx);
    let (glyph, color) = shown.glyph();
    // As `Value` in `Cjprb` draws it: the words, then a spacer that takes the room left, so the open arrow keeps its
    // gaps on both sides of the spacer however long the words are; words too long for the column end in an ellipsis.
    let value = Row::new()
      .id("task-property-merge")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Sm, color.clone()))
      .child(
        Text::new(&words)
          .id("task-property-merge-label")
          .variant(TypographyStyle::Description)
          .color(color)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)))
      .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted).id("task-property-merge-open"));
    let property = row(
      "task-property-merge",
      &ctx.t("summary.properties.merge.label"),
      value,
      TaskSpacing::PropertyRow.role(),
    );
    opening(open)
      .id("task-merge-row")
      .describe("a11y_name", words)
      .describe("state", shown.name())
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(ExtraSpacing::Tight.role()))
      .child(property)
      .into()
  }
}
