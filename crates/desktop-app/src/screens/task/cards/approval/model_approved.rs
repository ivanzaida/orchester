//! Decision-model approvals: the quiet answered line and its recorded verdict (06 a–b, xJ4Rf).
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{
  AnswerKind, ToolApprovalDecision,
  review::{DecisionVerdict, Review, Risk},
};
use orchester_workspace::{display, tasks::timeline::ApprovalCard};

use crate::{
  theme::{CardsText, ExtraRadius, ExtraSpacing, RISK_TRACK_HEIGHT},
  ui::{
    chat::AVATAR_SIZE,
    decision::{AnsweredLine, Mark},
    icon::Icon,
  },
};

/// Column widths in 06b (iAu5v, INogI).
const LABEL_WIDTH: f32 = 200.0;
const TRACK_WIDTH: f32 = 160.0;

pub(super) fn verdict(card: &ApprovalCard) -> Option<&DecisionVerdict> {
  if card.decided != Some(Some(ToolApprovalDecision::Approved))
    || card.decided_by != Some(orchester_core::run::DecidedBy::Reviewer)
    || card
      .answer
      .as_ref()
      .is_some_and(|answer| answer.kind != AnswerKind::Reviewer)
  {
    return None;
  }
  match card.review.as_ref()? {
    Review::Decided { decision } if decision.approves() => Some(decision),
    _ => None,
  }
}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub card: ApprovalCard,
}
impl lurq::app::component::DevtoolsInspectable for Props {}

pub(super) struct ModelApproved {
  open: Signal<bool>,
}

impl Component for ModelApproved {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let card = &ctx.props::<Props>().card;
    let Some(decision) = verdict(card) else {
      return Element::from(Column::new());
    };
    let id = format!("cards-approval-{}", card.request.id);
    let open = self.open.get();
    let toggle = self.open.clone();
    let time = card.decided_at.map_or_else(String::new, display::clock);
    let probability = format!("{:.2}", decision.approve.value());
    let lead = ctx.t_args(
      "cards.approval.modelApproved.lead",
      [("model", decision.model.as_str())],
    );
    let detail = ctx.t_args(
      "cards.approval.modelApproved.at",
      [("probability", probability.as_str()), ("time", time.as_str())],
    );
    let line = AnsweredLine::new(
      Mark::Reviewer,
      lead,
      Some(super::decided::asked(ctx, &card.request)),
      detail,
    )
    .render(
      &format!("{id}-line"),
      if open { Icon::ChevronDown } else { Icon::ChevronRight },
      move || toggle.update(|value| *value = !*value),
    );
    Row::new()
      .id(id.clone())
      .describe("waiting", "false")
      .describe("answered_as", "decision_model")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(Row::new().width(Dimension::Px(AVATAR_SIZE)).flex_shrink(0.0))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Sm)
          .child(line)
          .with_children(open.then(|| details(ctx, &id, decision))),
      )
      .into()
  }
}

fn details(ctx: &Ctx, id: &str, decision: &DecisionVerdict) -> Element {
  let missing = ctx.t("cards.approval.modelApproved.unknown");
  let elapsed = decision
    .elapsed_ms
    .map_or_else(|| missing.to_string(), |ms| ms.to_string());
  let model = ctx.t_args(
    "cards.approval.modelApproved.modelValue",
    [
      ("model", decision.model.as_str()),
      ("version", decision.version.as_deref().unwrap_or(&missing)),
      ("ms", elapsed.as_str()),
    ],
  );
  let approve = format!("{:.2}", decision.approve.value());
  let threshold = format!("{:.2}", decision.threshold.value());
  let value = ctx.t_args(
    "cards.approval.modelApproved.verdictValue",
    [("approve", approve.as_str()), ("threshold", threshold.as_str())],
  );
  Column::new()
    .id(format!("{id}-details"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceBase)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      property(ctx, "cards.approval.modelApproved.model").child(
        Text::new(&model)
          .id(format!("{id}-model"))
          .variant(CardsText::Lead.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      ),
    )
    .child(
      property(ctx, "cards.approval.modelApproved.verdict").child(
        Text::new(&value)
          .id(format!("{id}-verdict"))
          .variant(CardsText::AnsweredCommand.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ExtraSpacing::Hair.value()))
        .background(PaletteColor::Border),
    )
    .with_children(Risk::ALL.into_iter().enumerate().map(|(index, risk)| {
      risk_row(
        ctx,
        &format!("{id}-risk-{index}"),
        risk,
        decision.risks.of(risk).value(),
      )
    }))
    .into()
}

fn risk_row(ctx: &Ctx, id: &str, risk: Risk, probability: f64) -> Row {
  property(ctx, risk_key(risk))
    .id(id.to_owned())
    .child(
      Row::new()
        .width(Dimension::Px(TRACK_WIDTH))
        .height(Dimension::Px(RISK_TRACK_HEIGHT))
        .background(PaletteColor::SurfaceRaised)
        .rounded(ExtraRadius::Full.role())
        .child(
          Row::new()
            .id(format!("{id}-bar"))
            .width(Dimension::Pct((probability * 100.0) as f32))
            .height(Dimension::Pct(100.0))
            .background(PaletteColor::TextMuted)
            .rounded(ExtraRadius::Full.role()),
        ),
    )
    .child(
      Text::new(&format!("{probability:.2}"))
        .id(format!("{id}-value"))
        .variant(CardsText::RiskValue.role()),
    )
}
fn property(ctx: &Ctx, key: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(key))
        .variant(CardsText::Quiet.role())
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
}

const fn risk_key(risk: Risk) -> &'static str {
  match risk {
    Risk::WritesOutsideWorktree => "cards.approval.modelApproved.risk.outside",
    Risk::ReachesNetwork => "cards.approval.modelApproved.risk.network",
    Risk::ReadsCredential => "cards.approval.modelApproved.risk.credential",
    Risk::DiscardsWork => "cards.approval.modelApproved.risk.discard",
    Risk::DoesNotServeTask => "cards.approval.modelApproved.risk.task",
  }
}
