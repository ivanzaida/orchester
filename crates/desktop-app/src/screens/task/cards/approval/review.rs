//! Decision review line below a command or tool request (05 a–c, `JFEPS`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::run::review::{DecisionVerdict, Probability, Review, ReviewFailure, Risk};

use crate::{
  theme::{CardsSpacing, CardsText, ControlHeight, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

enum Content<'a> {
  Verdict(&'a DecisionVerdict),
  Failed(ReviewFailure, Option<&'a str>),
}

impl<'a> Content<'a> {
  fn of(review: Option<&'a Review>) -> Option<Self> {
    match review? {
      Review::Decided { decision } if !decision.approves() => Some(Self::Verdict(decision)),
      // A language reviewer's failure is said by the could-not-run line (ORC-611).
      failed @ Review::Failed { failure, model, .. } if failed.decision_failed() => {
        Some(Self::Failed(*failure, model.as_deref()))
      }
      _ => None,
    }
  }
}

pub(super) fn line(ctx: &Ctx, id: &str, review: Option<&Review>) -> Option<Element> {
  Some(match Content::of(review)? {
    Content::Verdict(decision) => verdict(ctx, id, decision),
    Content::Failed(failure, model) => failed(ctx, id, failure, model),
  })
}

/// Whether the decision reviewer left this request to the person (05 a–c).
pub(super) fn left_to_person(review: Option<&Review>) -> bool {
  Content::of(review).is_some()
}

/// The older review card's surface, preserved outside the decision-model shell.
pub(super) fn card(frame: Column, review: Option<&Review>) -> Column {
  if Content::of(review).is_some() {
    frame.background(PaletteColor::SurfacePanel)
  } else {
    frame
  }
}

fn frame(id: &str, glyph: Icon, color: PaletteColor) -> Row {
  Row::new()
    .id(format!("{id}-review"))
    .width(Dimension::Pct(100.0))
    .spacing(CardsSpacing::ReviewGap.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
}

fn words(id: &str, value: &str, color: PaletteColor) -> Text {
  Text::new(value)
    .id(id.to_owned())
    .variant(CardsText::Review.role())
    .color(color)
}

/// `XrNIE` and `uREHp`: no border, mono figures, 20 high, 6 at either side.
fn chip(id: &str, probability: Probability, risk: bool) -> Element {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(ControlHeight::Xs.value()))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .rounded(RadiusSize::Sm)
    .background(if risk {
      PaletteColor::WarningMuted
    } else {
      PaletteColor::SurfaceRaised
    })
    .align_items(Alignment::Center)
    .flex_shrink(0.0)
    .child(
      Text::new(&format!("{:.2}", probability.value()))
        .variant(CardsText::Probability.role())
        .nowrap(),
    )
    .into()
}

fn verdict(ctx: &Ctx, id: &str, decision: &DecisionVerdict) -> Element {
  let risk = decision.risks.named();
  let color = if risk.is_some() {
    PaletteColor::Warning
  } else {
    PaletteColor::TextMuted
  };
  let threshold = format!("{:.2}", decision.threshold.value());
  let under = ctx.t_args(
    if risk.is_some() {
      "cards.approval.decision.under"
    } else {
      "cards.approval.decision.noRisk"
    },
    [("threshold", threshold.as_str())],
  );
  let mut row = frame(id, Icon::Scale, color)
    .child(words(
      &format!("{id}-review-model"),
      &ctx.t_args("cards.approval.decision.left", [("model", decision.model.as_str())]),
      PaletteColor::TextSecondary,
    ))
    .child(chip(&format!("{id}-review-probability"), decision.approve, false))
    .child(words(
      &format!("{id}-review-threshold"),
      &under,
      PaletteColor::TextMuted,
    ));
  if let Some((risk, probability)) = risk {
    row = row
      .child(words(
        &format!("{id}-review-risk"),
        &ctx.t(risk_key(risk)),
        PaletteColor::TextPrimary,
      ))
      .child(chip(&format!("{id}-review-risk-probability"), probability, true));
  }
  row.into()
}

const fn risk_key(risk: Risk) -> &'static str {
  match risk {
    Risk::WritesOutsideWorktree => "cards.approval.decision.risk.outside",
    Risk::ReachesNetwork => "cards.approval.decision.risk.network",
    Risk::ReadsCredential => "cards.approval.decision.risk.credential",
    Risk::DiscardsWork => "cards.approval.decision.risk.discard",
    Risk::DoesNotServeTask => "cards.approval.decision.risk.task",
  }
}

fn failed(ctx: &Ctx, id: &str, failure: ReviewFailure, model: Option<&str>) -> Element {
  let model = model.map_or_else(|| ctx.t("cards.approval.decision.reviewer"), Into::into);
  let key = match failure {
    ReviewFailure::KeyRefused => "cards.approval.decision.failure.key",
    ReviewFailure::NothingAnswers => "cards.approval.decision.failure.unreachable",
    ReviewFailure::NotSystemOneApi => "cards.approval.decision.failure.api",
    ReviewFailure::ModelGone => "cards.approval.decision.failure.model",
    ReviewFailure::TimedOut => "cards.approval.decision.failure.timeout",
    ReviewFailure::Unreadable => "cards.approval.decision.failure.unreadable",
    ReviewFailure::CouldNotRun => "cards.approval.decision.failure.run",
    ReviewFailure::NoReviewer => "cards.approval.decision.failure.missing",
  };
  frame(id, Icon::TriangleAlert, PaletteColor::Warning)
    .child(words(
      &format!("{id}-review-failure"),
      &ctx.t_args(key, [("model", &*model)]),
      PaletteColor::TextSecondary,
    ))
    .child(words(
      &format!("{id}-review-settings"),
      &ctx.t("cards.approval.decision.settings"),
      PaletteColor::TextMuted,
    ))
    .into()
}
