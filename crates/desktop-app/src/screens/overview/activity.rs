//! The Overview's recent runs (`Recent activity` `BZ65j`): on a panel, the latest change of the most
//! recently changed runs, each its status's icon, what happened and its task with the time, opening the
//! task; a run of the merge queue as the queue has it (`Urfxj` in `K1d1mi`), and a blocked queue on top, opening the
//! Merge queue. Without any it says why (`nttN9`); while the tasks are read, a reading line (`GVk14`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Text},
  node::{Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  merge_queue::{Left, QueueState},
  run::TaskPhase,
};
use orchester_workspace::{
  display,
  tasks::{
    data::ProjectTasks,
    overview::{RecentRun, activity_empty, recent_activity},
  },
};

use super::{open_task, tasks::reading};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab},
  theme::{ExtraColor, ExtraSpacing, IconSize, OverviewText},
  ui::{
    card::{CardSurface, card_frame},
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    status::Status,
  },
};

/// Runs listed here.
const SHOWN: usize = 6;

/// The title and the runs; `tasks` is `None` while they are read.
pub(super) fn section(ctx: &mut Ctx, project: ProjectId, tasks: Option<&ProjectTasks>) -> Column {
  let section = Column::new()
    .id("overview-activity")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(Text::new(&ctx.t("overview.activity.title")).variant(TypographyStyle::Title));
  let Some(tasks) = tasks else {
    return section.child(reading(
      &ctx.t("overview.activity.reading"),
      "overview-activity-reading",
    ));
  };
  let recent = recent_activity(tasks, SHOWN);
  if recent.is_empty() {
    return section.child(
      Text::new(&message_text(ctx, &activity_empty(&tasks.phases)))
        .id("overview-activity-empty")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    );
  }
  let blocked = blocked_entry(ctx, project, tasks);
  let entries: Vec<Button> = blocked
    .into_iter()
    .chain(recent.iter().map(|run| entry(ctx, project, run)))
    .collect();
  section.child(
    card_frame("overview-activity-list", CardSurface::Panel)
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Xs))
      .with_children(entries),
  )
}

/// One run (`Activity row` `JUUSu`): *Run #128 completed*, *KON-9 · 13:12*.
fn entry(ctx: &mut Ctx, project: ProjectId, run: &RecentRun) -> Button {
  let status = Status::of_phase(run.phase);
  let (glyph, color) = queue_look(run).unwrap_or_else(|| (status.icon(), status.color()));
  let meta = ctx.t_args(
    "overview.activity.meta",
    [("task", run.key.to_string()), ("time", display::clock(run.at))],
  );
  opening(open_task(ctx, project, &run.key, TaskTab::Summary))
    .id(format!("overview-event-{}", run.key))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("status", status.key())
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(&message_text(ctx, &run.title()))
        .variant(OverviewText::Event.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Text::new(&meta).variant(OverviewText::FigureDetail.role()).nowrap())
}

/// The glyph of a run of the merge queue: waiting its turn, delivered, waiting for the person, or out of it.
fn queue_look(run: &RecentRun) -> Option<(Icon, PaletteColor)> {
  if run.phase != TaskPhase::Succeeded {
    return None;
  }
  Some(match &run.queue.as_ref()?.0 {
    QueueState::Queued => (Icon::Clock3, PaletteColor::TextSecondary),
    QueueState::Syncing | QueueState::Rechecking { .. } | QueueState::Integrating { .. } => {
      (Icon::LoaderCircle, PaletteColor::TextSecondary)
    }
    QueueState::WaitingForApproval => (Icon::Hand, PaletteColor::Warning),
    QueueState::Blocked(_) => (Icon::CirclePause, PaletteColor::Warning),
    QueueState::Left(left) if left.failed() => (Icon::CircleX, PaletteColor::Danger),
    QueueState::Left(Left::KeepsMoving { .. } | Left::Conflicts { .. } | Left::CheckFailed { .. }) => {
      (Icon::Hand, PaletteColor::Warning)
    }
    QueueState::Left(_) | QueueState::Merged(_) | QueueState::Removed => return None,
  })
}

/// *Merge queue blocked: uncommitted changes in your checkout*, at the time it blocked, opening the Merge queue.
fn blocked_entry(ctx: &mut Ctx, project: ProjectId, tasks: &ProjectTasks) -> Option<Button> {
  let since = tasks.merge_queue.iter().find_map(|entry| match &entry.state {
    QueueState::Blocked(_) => Some(entry.since),
    _ => None,
  })?;
  let route = Navigation::of(ctx).to(Route::Project(project, ProjectPage::MergeQueue));
  Some(
    opening(route)
      .id("overview-event-merge-queue")
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Sm))
      .spacing(ExtraSpacing::Loose.role())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .describe("status", "blocked")
      .child(icon(Icon::CirclePause, IconSize::Md, PaletteColor::Warning))
      .child(
        Text::new(&ctx.t("overview.activity.mergeQueueBlocked"))
          .variant(OverviewText::Event.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(
        Text::new(&display::clock(since))
          .variant(OverviewText::FigureDetail.role())
          .nowrap(),
      ),
  )
}
