//! Beside the overview's task list (`EKUWS`): what needs the user (`jWEn1`) — runs waiting for a
//! plan decision (a review card, `dpeD7`), an answer to an agent's question (`Mt22P`, which opens
//! the task's conversation) or an approval — and the recent activity of the project's runs
//! (`YJ8PV`, `mBkeJ` events). Each opens its task; they are outside any form, so pointer-only.

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
  navigation::route::{Page, TaskPageTab},
  tasks::{
    data::ProjectTasks,
    list::{AwaitedPlan, TaskRow},
    overview::{OverviewTab, RecentRun, activity_empty, figure, recent_runs},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
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

/// A run waiting for the user: its plan to review (`dpeD7`), or else what it waits for (`Mt22P`):
/// the question an agent asked, with the way to the task's conversation, or its task.
fn decision(ctx: &Ctx, row: &TaskRow, nav: &ProjectNav) -> Element {
  if let Some(plan) = &row.plan {
    return plan_decision(ctx, row, plan, nav);
  }
  let (nav, task) = (nav.clone(), row.id);
  let (overline, title, context, action, page) = match &row.question {
    Some(question) => (
      ctx.t("overview.attention.clarification"),
      question.text.clone(),
      ctx.t_args(
        "overview.attention.questionContext",
        [("agent", &*question.agent), ("task", &*row.key)],
      ),
      ctx.t("overview.attention.answer"),
      Page::Task(task, TaskPageTab::Conversation),
    ),
    None => (
      ctx.t("overview.attention.needsInput"),
      row.title.clone(),
      ctx.t_args(
        "overview.attention.context",
        [("task", &*row.key), ("detail", &*message_text(ctx, &row.detail))],
      ),
      ctx.t("overview.attention.open"),
      Page::task(task),
    ),
  };
  pressable(surface().id(format!("overview-decision-{}", row.key)), move || {
    nav.show(page);
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
      .child(Text::new(&overline).variant(ExtraTypography::Overline.role()))
      .child(
        Text::new(&title)
          .variant(ExtraTypography::NoticeHeading.role())
          .width(Dimension::Pct(100.0)),
      )
      .child(Text::new(&context).variant(TypographyStyle::Caption))
      .child(Text::new(&action).variant(TypographyStyle::Link)),
  )
  .into()
}

/// A plan waiting for the user's decision (`dpeD7`): "REVIEW PLAN", the task, what waits, and
/// Review plan, which opens the task on its plan.
fn plan_decision(ctx: &Ctx, row: &TaskRow, plan: &AwaitedPlan, nav: &ProjectNav) -> Element {
  let version = plan.version.to_string();
  let summary = ctx.t_args(
    "overview.attention.planSummary",
    [("version", version.as_str()), ("agent", &*plan.agent)],
  );
  let (nav, task) = (nav.clone(), row.id);
  let review = ActionButton::new(ctx.t("overview.attention.reviewPlanAction"))
    .icon(Icon::ListChecks)
    .fill_width()
    .on_click(move || nav.show(Page::task(task)))
    .render(ctx)
    .id(format!("overview-review-plan-{}", row.key));
  Column::new()
    .id(format!("overview-decision-{}", row.key))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("overview.attention.reviewPlan")).variant(ExtraTypography::Overline.role()))
    .child(Text::new(&row.title).variant(TypographyStyle::Label))
    .child(
      Text::new(&summary)
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(review)
    .into()
}

/// Recent activity (`YJ8PV`): the latest change of the most recently changed runs.
fn activity(ctx: &Ctx, open: &OpenProject, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let now = Timestamp::now();
  let recent = recent_runs(&tasks.tasks, &tasks.phases, SHOWN);
  let events: Vec<Element> = if recent.is_empty() {
    vec![
      Text::new(&message_text(ctx, &activity_empty(&tasks.phases)))
        .id("overview-activity-empty")
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
