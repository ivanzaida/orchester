//! The Overview's recent runs (`Recent activity` `BZ65j`): on a panel, the latest change of the most
//! recently changed runs, each its status's icon, what happened and its task with the time, opening the
//! task. Without any it says why (`nttN9`); while the tasks are read, a reading line (`GVk14`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Text},
  node::{Style, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  display,
  tasks::{
    data::ProjectTasks,
    overview::{RecentRun, activity_empty, recent_runs},
  },
};

use super::{open_task, tasks::reading};
use crate::{
  routes::TaskTab,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    card::{CardSurface, card_frame},
    icon::icon,
    message::message_text,
    pressable::pressable,
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
  let recent = recent_runs(&tasks.tasks, &tasks.phases, SHOWN);
  if recent.is_empty() {
    return section.child(
      Text::new(&message_text(ctx, &activity_empty(&tasks.phases)))
        .id("overview-activity-empty")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    );
  }
  let entries: Vec<Button> = recent.iter().map(|run| entry(ctx, project, run)).collect();
  section.child(
    card_frame("overview-activity-list", CardSurface::Panel)
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Xs))
      .with_children(entries),
  )
}

/// One run (`Activity row` `JUUSu`): *Run #128 completed*, *KON-9 · 13:12*.
fn entry(ctx: &mut Ctx, project: ProjectId, run: &RecentRun) -> Button {
  let status = Status::of_phase(run.phase);
  let meta = ctx.t_args(
    "overview.activity.meta",
    [("task", run.key.to_string()), ("time", display::clock(run.at))],
  );
  pressable(open_task(ctx, project, &run.key, TaskTab::Summary))
    .id(format!("overview-event-{}", run.key))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("status", status.key())
    .child(icon(status.icon(), IconSize::Md, status.color()))
    .child(
      Text::new(&message_text(ctx, &run.title()))
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Text::new(&meta).variant(TypographyStyle::Caption).nowrap())
}
