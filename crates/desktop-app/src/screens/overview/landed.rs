//! *Landed* (`Landed` `Gi9sx` in 01 `LXhun`): the merge queue's state on a `surface_raised` line with *Review ↗* to
//! Review › Merge queue, then *Today · N merged* and the runs merged today (up to five, then *Show all N in Done*), and
//! *Earlier* with up to three of the week before, greyed (`orchester_workspace::overview::landed`). Landed counts
//! merges only. *Done ↗* opens Tasks on Done; a line opens its task's Summary.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use orchester_workspace::{
  display::{self, WEEKDAY_KEYS},
  messages::Message,
  overview::{
    EARLIER_SHOWN, TODAY_SHOWN,
    landed::{Landed, LandedRun},
  },
};

use super::section::{SIDE_LINE_HEIGHT, head, key, line, link, mono_time, muted, section, show_all, title};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, opening},
  },
};

/// The merge queue's line (`dllGX`).
const QUEUE_HEIGHT: f32 = 34.0;

/// The section, with the merge queue's state and how many wait behind it.
pub(super) fn landed(
  ctx: &mut Ctx,
  project: ProjectId,
  (landed, queue): (&Landed, (Message, Option<Message>)),
) -> Column {
  let navigation = Navigation::of(ctx);
  let done = navigation.to(Route::Project(
    project,
    ProjectPage::Tasks(TaskList::new(TaskListTab::Done)),
  ));
  let section = section("overview-landed")
    .child(head(
      "overview-landed",
      &ctx.t("overview.landed.title"),
      None,
      Some(link(
        "overview-landed-done",
        &ctx.t("overview.landed.done"),
        Press::Open(done.clone()),
      )),
    ))
    .child(queue_line(ctx, project, queue));
  let count = landed.today.len();
  let today_label = if count == 1 {
    ctx.t("overview.landed.today.one")
  } else {
    ctx.t_args("overview.landed.today.other", [("count", count.to_string())])
  };
  let mut section = section.child(sub("overview-landed-today", &today_label));
  for run in landed.today.iter().take(TODAY_SHOWN) {
    section = section.child(run_line(ctx, project, run, false));
  }
  if count > TODAY_SHOWN {
    let label = ctx.t_args("overview.landed.showAll", [("count", count.to_string())]);
    section = section.child(show_all("overview-landed-all", &label, Press::Open(done)));
  }
  if landed.earlier.is_empty() {
    return section;
  }
  section = section.child(sub("overview-landed-earlier", &ctx.t("overview.landed.earlier")));
  for run in landed.earlier.iter().take(EARLIER_SHOWN) {
    section = section.child(run_line(ctx, project, run, true));
  }
  section
}

/// The merge queue: its state, how many wait, and *Review ↗*.
fn queue_line(ctx: &mut Ctx, project: ProjectId, (state, waiting): (Message, Option<Message>)) -> Row {
  let state = message_text(ctx, &state);
  let meta = match waiting {
    Some(waiting) => ctx.t_args(
      "overview.landed.queueWaiting",
      [
        ("state", state.to_string()),
        ("waiting", message_text(ctx, &waiting).to_string()),
      ],
    ),
    None => state,
  };
  let open = Navigation::of(ctx).to(Route::Project(project, ProjectPage::MergeQueue));
  Row::new()
    .id("overview-merge-queue")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(QUEUE_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::GitPullRequest, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("overview.landed.queue"))
        .variant(OverviewText::Line.role())
        .nowrap(),
    )
    .child(muted(&meta).id("overview-merge-queue-state"))
    .child(Spacer::new().flex(1.0))
    .child(link(
      "overview-merge-queue-review",
      &ctx.t("overview.landed.review"),
      Press::Open(open),
    ))
}

/// A group's name over its runs (`zu9ca`): 8 over it, 2 under.
fn sub(id: &str, words: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Tight.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .child(
      Text::new(words)
        .id(id.to_owned())
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
}

/// A merged run: today's at its time, an earlier one greyed at its day.
fn run_line(ctx: &mut Ctx, project: ProjectId, run: &LandedRun, earlier: bool) -> Element {
  let id = format!("overview-landed-{}", run.key);
  let (mark, words, when) = if earlier {
    (
      PaletteColor::TextMuted,
      PaletteColor::TextSecondary,
      weekday(ctx, run.at),
    )
  } else {
    (PaletteColor::Success, PaletteColor::TextPrimary, display::clock(run.at))
  };
  let row = line(&id, SIDE_LINE_HEIGHT, ExtraSpacing::Loose.role())
    .child(icon(Icon::GitMerge, IconSize::Base, mark))
    .child(key(&run.key))
    .child(title(
      &format!("{id}-title"),
      &run.title,
      OverviewText::Line.role(),
      words,
    ))
    .child(mono_time(&format!("{id}-time"), &when));
  let task: TaskKey = run.key.parse().expect("a task's key reads back as a task key");
  let route = Route::Project(project, ProjectPage::Task(task, TaskView::of(TaskTab::Summary)));
  opening(Navigation::of(ctx).to(route))
    .id(format!("{id}-open"))
    .width(Dimension::Pct(100.0))
    .child(row)
    .into()
}

/// The local weekday of `at` (*Mon*).
fn weekday(ctx: &Ctx, at: Timestamp) -> String {
  message_text(ctx, &Message::new(WEEKDAY_KEYS[display::weekday(at)])).to_string()
}

/// The section's skeleton head while the tasks are read: its head and the merge queue's name.
pub(super) fn reading(ctx: &Ctx) -> Column {
  section("overview-landed").child(head("overview-landed", &ctx.t("overview.landed.title"), None, None))
}
