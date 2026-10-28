//! The quiet Overview (02 `HWznK`): nothing waits and nothing runs. In the digest's place, a `border` card radius 10
//! with a `circle-check` in `success`, *Nothing waits for you, and nothing is running.* and what is known of the slots,
//! the queue and the last merge (`bNnoJ`; its clock time today, how long ago before); under it *Ready to start*, the
//! drafts a person could queue now (`orchester_workspace::overview::ready`), each with *Ready* and its pipeline
//! (`aM7e8`), *Tasks ↗* opening the list, and *Show all N* where the list is cut.

use std::num::NonZeroU32;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use orchester_workspace::{
  display,
  overview::{QUEUE_SHOWN, ready::ready_to_start as ready_drafts},
  tasks::{data::ProjectTasks, list::PipelineLabel},
};

use super::section::{LINE_HEIGHT, head, key, line, link, muted, section, show_all, title};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView},
  theme::{ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, opening},
  },
};

/// The pipeline's room (`PzN8w`).
const PIPELINE_WIDTH: f32 = 150.0;

/// The card; `slots` as the runner reported them, `last_merge` the newest merge known, at `now`.
pub(super) fn nothing_waits(
  ctx: &Ctx,
  slots: Option<(usize, NonZeroU32)>,
  (last_merge, now): (Option<Timestamp>, Timestamp),
) -> Column {
  let mut parts: Vec<String> = Vec::new();
  if let Some((used, slots)) = slots {
    parts.push(
      ctx
        .t_args(
          "overview.quiet.slots",
          [("used", used.to_string()), ("slots", slots.to_string())],
        )
        .to_string(),
    );
  }
  parts.push(ctx.t("overview.quiet.queueEmpty").to_string());
  if let Some(at) = last_merge {
    let time = if at >= now.local_day_start() {
      display::clock(at)
    } else {
      message_text(ctx, &display::ago(now, at)).to_string()
    };
    parts.push(ctx.t_args("overview.quiet.lastMerge", [("time", time)]).to_string());
  }
  let note = parts.join(&ctx.t("overview.separator"));
  Column::new()
    .id("overview-quiet")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Start)
        .child(icon(Icon::CircleCheck, IconSize::Nav, PaletteColor::Success))
        .child(
          Column::new()
            .flex(1.0)
            .spacing(SpacingSize::Xs)
            .child(
              Text::new(&ctx.t("overview.quiet.title"))
                .id("overview-quiet-title")
                .variant(ExtraTypography::Strong.role()),
            )
            .child(muted(&note).id("overview-quiet-note")),
        ),
    )
}

/// *Ready to start*: the drafts that could be queued now; `None` when there are none.
pub(super) fn ready_to_start(ctx: &mut Ctx, project: ProjectId, tasks: &ProjectTasks) -> Option<Column> {
  let ready = ready_drafts(tasks);
  if ready.is_empty() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let all = navigation.to(Route::Project(
    project,
    ProjectPage::Tasks(TaskList::new(TaskListTab::Backlog)),
  ));
  let more = (ready.len() > QUEUE_SHOWN).then(|| {
    let label = ctx.t_args("overview.ready.showAll", [("count", ready.len().to_string())]);
    show_all("overview-ready-all", &label, Press::Open(all.clone()))
  });
  let lines: Vec<Element> = ready
    .iter()
    .take(QUEUE_SHOWN)
    .map(|task| {
      let task_key = task.key().to_string();
      let id = format!("overview-ready-{task_key}");
      let pipeline = PipelineLabel::of(task.details().pipeline, &tasks.pipelines).message();
      let row = line(&id, LINE_HEIGHT, SpacingSize::Md)
        .child(icon(Icon::Circle, IconSize::Md, PaletteColor::TextMuted))
        .child(key(&task_key))
        .child(title(
          &format!("{id}-title"),
          task.details().title.as_str(),
          ExtraTypography::Strong.role(),
          PaletteColor::TextPrimary,
        ))
        .child(
          Text::new(&ctx.t("overview.ready.ready"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        )
        .child(
          Row::new()
            .width(Dimension::Px(PIPELINE_WIDTH))
            .flex_shrink(0.0)
            .justify(Justify::End)
            .child(muted(&message_text(ctx, &pipeline)).text_overflow(TextOverflow::Elipsis)),
        );
      let key: TaskKey = task_key.parse().expect("a task's key reads back as a task key");
      let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::default())));
      opening(navigation.to(route))
        .id(format!("{id}-open"))
        .width(Dimension::Pct(100.0))
        .child(row)
        .into()
    })
    .collect();
  Some(
    section("overview-ready")
      .child(head(
        "overview-ready",
        &ctx.t("overview.ready.title"),
        Some(ready.len().to_string()),
        Some(link(
          "overview-ready-tasks",
          &ctx.t("overview.ready.tasks"),
          Press::Open(all),
        )),
      ))
      .with_children(lines)
      .with_children(more),
  )
}
