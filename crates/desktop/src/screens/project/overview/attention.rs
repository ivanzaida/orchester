//! Beside the overview's task list (`EKUWS`): what needs the user (`jWEn1`) — runs waiting for a
//! plan decision, an answer or an approval — and the recent activity of the project's runs
//! (`YJ8PV`, `mBkeJ` events). Rows open their task; they are outside any form, so pointer-only.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::super::{ProjectNav, task_rows::status_copy};
use crate::{
  display,
  navigation::route::Page,
  tasks::{
    data::ProjectTasks,
    list::TaskRow,
    overview::{OverviewTab, RecentRun, figure, recent_runs},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
  },
  workspace::OpenProject,
};

/// Panel width, heading height, event height and its glyph tile (`EKUWS`, `jWEn1`, `qbOVa`,
/// `tWFDE`).
const PANEL_WIDTH: f32 = 288.0;
const HEADING_HEIGHT: f32 = 34.0;
const EVENT_HEIGHT: f32 = 54.0;
const GLYPH_SIZE: f32 = 28.0;
/// Waiting runs and recent events shown; the task list has the rest.
const SHOWN: usize = 3;

pub(super) fn attention(ctx: &Ctx, open: &OpenProject, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let waiting = OverviewTab::NeedsYou.rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, SHOWN);
  let count = OverviewTab::NeedsYou.count(&tasks.tasks, &tasks.phases);
  let decisions: Vec<Element> = if waiting.is_empty() {
    vec![
      Text::new(&ctx.t("overview.attention.empty"))
        .variant(ExtraTypography::Detail.role())
        .into(),
    ]
  } else {
    waiting.iter().map(|row| decision(ctx, row, nav)).collect()
  };
  Column::new()
    .id("overview-attention")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(SpacingSize::Lg)
    .spacing(SpacingSize::Lg)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(HEADING_HEIGHT))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::Inbox, IconSize::Option, PaletteColor::TextPrimary))
        .child(Text::new(&ctx.t("overview.attention.title")).variant(ExtraTypography::CardTitle.role()))
        .child(Spacer::new().flex(1.0))
        .child(Text::new(&figure(count)).variant(TypographyStyle::Caption)),
    )
    .with_children(decisions)
    .child(activity(ctx, open, tasks, nav))
    .into()
}

/// A run waiting for the user (`Mt22P`): what it is, the task, and the way to it.
fn decision(ctx: &Ctx, row: &TaskRow, nav: &ProjectNav) -> Element {
  let (nav, task) = (nav.clone(), row.id);
  let context = ctx.t_args(
    "overview.attention.context",
    [("task", &*row.key), ("detail", &*message_text(ctx, &row.detail))],
  );
  pressable(surface().id(format!("overview-decision-{}", row.key)), move || {
    nav.show(Page::task(task));
  })
  .width(Dimension::Pct(100.0))
  .child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .padding_top(ExtraSpacing::Step.role())
      .padding_bottom(SpacingSize::Lg)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("overview.attention.needsInput")).variant(ExtraTypography::Overline.role()))
      .child(Text::new(&row.title).variant(ExtraTypography::NoticeHeading.role()))
      .child(Text::new(&context).variant(TypographyStyle::Caption))
      .child(Text::new(&ctx.t("overview.attention.open")).variant(TypographyStyle::Link)),
  )
  .into()
}

/// Recent activity (`YJ8PV`): the latest change of the most recently changed runs.
fn activity(ctx: &Ctx, open: &OpenProject, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let now = Timestamp::now();
  let recent = recent_runs(&tasks.tasks, &tasks.phases, SHOWN);
  let events: Vec<Element> = if recent.is_empty() {
    vec![
      Text::new(&ctx.t("overview.activity.empty"))
        .variant(TypographyStyle::Caption)
        .into(),
    ]
  } else {
    recent
      .iter()
      .map(|run| event(ctx, open.project.name().as_str(), run, now, nav))
      .collect()
  };
  Column::new()
    .id("overview-activity")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Divider.role())
    .child(Text::new(&ctx.t("overview.activity.title")).variant(TypographyStyle::Label))
    .with_children(events)
    .into()
}

/// One activity event (`mBkeJ`): glyph, "Run #128 completed", "ORC-12 · Kontur · 2m ago".
fn event(ctx: &Ctx, project: &str, run: &RecentRun, now: Timestamp, nav: &ProjectNav) -> Element {
  let (glyph, _) = status_copy(run.phase);
  let elapsed = message_text(ctx, &display::elapsed(now, run.at));
  let detail = ctx.t_args(
    "overview.activity.detail",
    [("task", &*run.key), ("project", project), ("time", &*elapsed)],
  );
  let tile = Row::new()
    .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextSecondary));
  let (nav, task) = (nav.clone(), run.task);
  pressable(surface().id(format!("overview-event-{}", run.key)), move || {
    nav.show(Page::task(task));
  })
  .width(Dimension::Pct(100.0))
  .child(
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(EVENT_HEIGHT))
      .spacing(ExtraSpacing::Stack.role())
      .child(tile)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&message_text(ctx, &run.title()))
              .variant(TypographyStyle::Caption)
              .color(PaletteColor::TextPrimary),
          )
          .child(Text::new(&detail).variant(ExtraTypography::Footnote.role())),
      ),
  )
  .into()
}
