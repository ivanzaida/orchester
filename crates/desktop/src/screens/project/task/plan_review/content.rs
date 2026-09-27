//! The plan and the decision (`U2JBf`): the notice that the approval is needed (`z1V7b`), the plan's
//! title (`J7irj`) and its text as its agent wrote it, then *Request changes* and *Approve plan &
//! continue* (`XXsKR`) with what approving does (`L03Wv0`). The plan scrolls in a viewport with a
//! scroll gutter; the decision stays at the bottom. An earlier version reads without a decision.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor, SpacingSize},
  },
  components::{Column, Markdown, MarkdownLink, MarkdownProps, Row, ScrollVertical, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{Plan, PlanDecision};

use super::{Choice, Decisions, PlanReviewProps};
use crate::{
  external,
  tasks::plan::PlanReview,
  theme::{ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{button::ActionButton, field::field_error, icon::Icon, message_text, notice::Notice},
};

pub(super) fn content(ctx: &mut Ctx, props: &PlanReviewProps, shown: &Plan, decisions: &Decisions) -> Element {
  let review = &props.review;
  let version = shown.version.to_string();
  let title = ctx.t_args(
    "runs.plan.title",
    [("version", version.as_str()), ("task", &*props.task)],
  );
  let plan = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(notice(ctx, review))
    .child(
      Text::new(&title)
        .id("plan-review-title")
        .variant(ExtraTypography::PlanTitle.role()),
    )
    .child(plan_text(ctx, &shown.text));
  let decision = if shown.version == review.waiting().version {
    decision(ctx, review, decisions)
  } else {
    earlier(ctx, shown, review.waiting())
  };
  // The plan's viewport keeps a scroll gutter at its right; the decision below it ends where the
  // gutter does, 32 from the edge like the plan.
  Column::new()
    .id("plan-review-content")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Section)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::BesideGutter.role())
    .spacing(ExtraSpacing::Step.role())
    .child(
      ScrollVertical::new(plan)
        .id("plan-review-plan")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_right(ExtraSpacing::Stack.role())
        .spacing(ExtraSpacing::Step.role())
        .with_children(decision),
    )
    .into()
}

/// "Your approval is needed" · "Review plan v1 from Planner. Builder has not started." (`z1V7b`).
fn notice(ctx: &Ctx, review: &PlanReview) -> Element {
  let version = review.waiting().version.to_string();
  let next = review.next.join(&*ctx.t("runs.progress.together"));
  let key = match review.next.len() {
    0 => "runs.plan.notice.detail.none",
    1 => "runs.plan.notice.detail.one",
    _ => "runs.plan.notice.detail.other",
  };
  let detail = ctx.t_args(
    key,
    [
      ("version", version.as_str()),
      ("agent", &*review.agent),
      ("next", next.as_str()),
    ],
  );
  Notice::new(Icon::Hand, ctx.t("runs.plan.notice.title"), detail)
    .id("plan-review-notice")
    .into()
}

/// The plan as Markdown in the agent text style (`FP6BD`); https links open in the browser.
fn plan_text(ctx: &mut Ctx, text: &str) -> Element {
  if text.trim().is_empty() {
    return Text::new(&ctx.t("runs.plan.empty"))
      .id("plan-review-text")
      .variant(ExtraTypography::Detail.role())
      .into();
  }
  let style = {
    let theme = ctx.theme();
    FormTextRole {
      typography: ExtraTypography::AgentText.role(),
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let props = MarkdownProps::styled(Arc::<str>::from(text), style)
    .width(Dimension::Pct(100.0))
    .selectable(true)
    .on_link_click(|link: &MarkdownLink| {
      if let Err(error) = external::open_url(link.destination()) {
        tracing::warn!(%error, "the plan's link was not opened");
      }
    });
  Column::new()
    .id("plan-review-text")
    .width(Dimension::Pct(100.0))
    .child(Markdown::mount(ctx, props))
    .into()
}

/// Request changes and Approve plan & continue (`XXsKR`), why a decision failed, and what
/// approving does (`L03Wv0`).
fn decision(ctx: &Ctx, review: &PlanReview, decisions: &Decisions) -> Vec<Element> {
  // Requesting changes needs a note saying what should change.
  let request = match (decisions.pending, &decisions.request_changes) {
    (Some(Choice::RequestChanges), _) => ActionButton::new(ctx.t("runs.plan.requestingChanges"))
      .icon(Icon::Loader)
      .disabled(),
    (_, Some(press)) => {
      let press = press.clone();
      ActionButton::new(ctx.t("runs.plan.requestChanges"))
        .icon(Icon::MessageSquare)
        .on_click(move || press())
    }
    (_, None) => ActionButton::new(ctx.t("runs.plan.requestChanges"))
      .icon(Icon::MessageSquare)
      .disabled(),
  };
  let approve = match (decisions.pending, &decisions.approve) {
    (Some(Choice::Approve), _) => ActionButton::new(ctx.t("runs.plan.approving"))
      .icon(Icon::Loader)
      .primary()
      .disabled(),
    (_, Some(press)) => {
      let press = press.clone();
      ActionButton::new(ctx.t("runs.plan.approve"))
        .icon(Icon::Check)
        .primary()
        .on_click(move || press())
    }
    (_, None) => ActionButton::new(ctx.t("runs.plan.approve"))
      .icon(Icon::Check)
      .primary()
      .disabled(),
  };
  let next = review.next.join(&*ctx.t("runs.progress.together"));
  let meaning = match review.next.len() {
    0 => ctx.t("runs.plan.approveMeaning.none"),
    1 => ctx.t_args("runs.plan.approveMeaning.one", [("next", next.as_str())]),
    _ => ctx.t_args("runs.plan.approveMeaning.other", [("next", next.as_str())]),
  };
  let actions = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(request.id("plan-request-changes").in_form().render(ctx))
    .child(Spacer::new().flex(1.0))
    .child(approve.id("plan-approve").in_form().render(ctx));
  let failure = decisions
    .failure
    .as_ref()
    .map(|failure| field_error(&message_text(ctx, failure)));
  std::iter::once(actions.into())
    .chain(failure)
    .chain(std::iter::once(
      Text::new(&meaning)
        .id("plan-approve-meaning")
        .variant(ExtraTypography::Footnote.role())
        .into(),
    ))
    .collect()
}

/// An earlier version reads without a decision: what became of it, and which one waits.
fn earlier(ctx: &Ctx, shown: &Plan, waiting: &Plan) -> Vec<Element> {
  let (version, latest) = (shown.version.to_string(), waiting.version.to_string());
  let args = [("version", version.as_str()), ("latest", latest.as_str())];
  let mut lines = vec![ctx.t_args("runs.plan.earlier", args)];
  if let Some(PlanDecision::ChangesRequested { note }) = shown.decision() {
    lines.push(ctx.t_args("runs.plan.earlierNote", [("note", note.as_str())]));
  }
  lines
    .into_iter()
    .map(|line| {
      Text::new(&line)
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0))
        .into()
    })
    .collect()
}
