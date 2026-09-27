//! Beside the plan (`e1dTtW`): the task's acceptance criteria, the note for the agent with what
//! requesting changes does (`N8bUI9`, `NbNN0`, `biSnS`), and at the bottom the run's tokens and the
//! plan's version (`RCDJQ`, `d1NvIW`) — a choice of the step's versions once there are several.
//! The details scroll in a viewport with a scroll gutter; the properties stay at the bottom.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, ScrollVertical, Select, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::run::Plan;

use super::{PlanReviewProps, Shown};
use crate::{
  tasks::plan::{PlanReview, state_key},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    field::{FieldNote, field, select_style, styled_input},
    property::property,
  },
};

/// Panel width (`e1dTtW`), the note field's height (`NbNN0`) and the version choice's height (the
/// buttons' height, `u1XBP`).
const ASIDE_WIDTH: f32 = 360.0;
const NOTE_HEIGHT: f32 = 132.0;
const VERSION_HEIGHT: f32 = 36.0;

pub(super) fn aside(
  ctx: &Ctx,
  props: &PlanReviewProps,
  note: &Signal<String>,
  note_focused: &Signal<bool>,
  shown: &Signal<Shown>,
) -> Element {
  let review = &props.review;
  let criteria: Vec<Element> = if props.criteria.is_empty() {
    vec![detail(ctx.t("runs.plan.noCriteria").as_ref())]
  } else {
    props
      .criteria
      .iter()
      .map(|criterion| detail(&ctx.t_args("runs.plan.criterion", [("criterion", criterion.as_str())])))
      .collect()
  };
  let details = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Text::new(&ctx.t("runs.plan.criteria")).variant(ExtraTypography::CardTitle.role()))
    .with_children(criteria)
    .child(
      Text::new(&ctx.t_args("runs.plan.changesFor", [("agent", &*review.agent)]))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    )
    .child(note_field(ctx, note, note_focused))
    .child(detail(&changes_meaning(ctx, review)));
  let properties = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .child(property(&ctx.t("runs.plan.tokensUsed"), &props.tokens))
    .child(version(ctx, review, shown));
  Column::new()
    .id("plan-review-aside")
    .width(Dimension::Px(ASIDE_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_left(SpacingSize::Lg)
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ScrollVertical::new(details)
        .id("plan-review-details")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(properties)
    .into()
}

fn detail(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Detail.role())
    .width(Dimension::Pct(100.0))
    .into()
}

/// "Request changes returns the plan to Planner. Builder stays blocked." (`biSnS`).
fn changes_meaning(ctx: &Ctx, review: &PlanReview) -> Arc<str> {
  let next = review.next.join(&*ctx.t("runs.progress.together"));
  let key = match review.next.len() {
    0 => "runs.plan.changesMeaning.none",
    1 => "runs.plan.changesMeaning.one",
    _ => "runs.plan.changesMeaning.other",
  };
  ctx.t_args(key, [("agent", &*review.agent), ("next", next.as_str())])
}

/// The note for the agent (`NbNN0`): a text area on the surface fill, with the focused field stroke
/// (`o8Oug`) while it has the focus.
fn note_field(ctx: &Ctx, note: &Signal<String>, focused: &Signal<bool>) -> Element {
  let input = ctx.theme().form().input.clone();
  let border = if focused.get() {
    Border::inside(BorderSize::Md, input.border_focus)
  } else {
    Border::inside(BorderSize::Sm, input.border)
  };
  let (gained, lost) = (focused.clone(), focused.clone());
  let area = styled_input(ctx, note.clone(), ExtraTypography::ReviewNote.role())
    .id("plan-review-note")
    .textarea()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .placeholder(&ctx.t("runs.plan.notePlaceholder"))
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false));
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(NOTE_HEIGHT))
    .padding(ExtraSpacing::TextArea.role())
    .background(ExtraColor::Surface.role())
    .border(border)
    .rounded(ExtraRadius::Notice.role())
    .child(area)
    .into()
}

/// "Plan version · v1 · awaiting approval" (`d1NvIW`); with several versions, a choice of them,
/// newest first.
fn version(ctx: &Ctx, review: &PlanReview, shown: &Signal<Shown>) -> Element {
  let label = ctx.t("runs.plan.planVersion");
  let plans = review.plans();
  if plans.len() < 2 {
    return property(&label, &version_label(ctx, review.waiting()));
  }
  let options = plans.iter().rev().map(|plan| {
    let value = if plan.version == review.waiting().version {
      Shown(None)
    } else {
      Shown(Some(plan.version))
    };
    (value, version_label(ctx, plan))
  });
  let select = Element::from(
    Select::new(shown.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(VERSION_HEIGHT))
      .style(select_style(ctx, VERSION_HEIGHT)),
  )
  .id("plan-review-version");
  field(&label, select, FieldNote::None)
}

fn version_label(ctx: &Ctx, plan: &Plan) -> Arc<str> {
  let version = plan.version.to_string();
  ctx.t_args(state_key(plan), [("version", version.as_str())])
}
