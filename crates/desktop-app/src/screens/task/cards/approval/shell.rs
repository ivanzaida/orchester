//! The 06b approval shell around decision-review lines (05 JFEPS).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::display;

use super::{
  super::Place,
  ApprovalView,
  answers::{self, Answering},
  message_inline, request, review,
};
use crate::{
  theme::{CardsSpacing, CardsText, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    clipboard::Clipboard,
    decision::{AnsweredLine, Mark},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

pub(super) fn card(
  ctx: &mut Ctx,
  view: &ApprovalView,
  place: &Place,
  answering: &Answering<'_>,
  editing: bool,
  failed: Option<Element>,
) -> Element {
  let id = answering.id;
  let question = question(ctx, answering);
  if view.folded.get() {
    let unfold = view.folded.clone();
    let time = display::clock(answering.request.requested_at);
    let detail = ctx.t_args(
      "cards.foldedDetail",
      [("question", &*question), ("time", time.as_str())],
    );
    let line = AnsweredLine::new(Mark::Waiting, ctx.t("cards.folded"), None, detail).render(
      &format!("{id}-line"),
      Icon::ChevronDown,
      move || unfold.set(false),
    );
    return Column::new()
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .child(line)
      .into();
  }
  let actions = if editing {
    message_inline::editor(ctx, answering)
  } else {
    vec![answers::inline_answers(ctx, answering, &view.hint)]
  };
  let card = Column::new()
    .id(format!("{id}-card"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      CardsSpacing::InlineSides.role(),
      CardsSpacing::InlineEnds.role(),
    ))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .spacing(SpacingSize::Lg)
    .child(top(ctx, view, answering, &question))
    .with_children(actions)
    .with_children(failed);
  Column::new()
    .id(id.to_owned())
    .describe("waiting", "true")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(card)
    .child(meta(ctx, place, answering, &question))
    .into()
}

fn top(ctx: &Ctx, view: &ApprovalView, answering: &Answering<'_>, question: &str) -> Row {
  let id = answering.id;
  let fold = view.folded.clone();
  let close = pressable(move || fold.set(true))
    .id(format!("{id}-fold"))
    .describe("a11y_name", ctx.t("cards.fold"))
    .child(icon(Icon::X, IconSize::Lg, PaletteColor::TextMuted));
  let context = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .child(command(answering))
    .with_children(review::line(
      ctx,
      id,
      answering.shown.and_then(|shown| shown.review.as_ref()),
    ))
    .child(
      Text::new(question)
        .id(format!("{id}-question"))
        .variant(CardsText::Decision.role())
        .width(Dimension::Pct(100.0)),
    );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(context)
    .child(close)
}

fn question(ctx: &Ctx, answering: &Answering<'_>) -> std::sync::Arc<str> {
  ctx.t_args(
    if answering.request.command {
      "cards.approval.inline.command"
    } else {
      "cards.approval.inline.tool"
    },
    [("agent", answering.request.agent.as_str())],
  )
}

fn command(answering: &Answering<'_>) -> Element {
  let request = answering.request;
  let words = if request.command {
    request.action.clone()
  } else {
    format!(
      "{} · {}  {}",
      request.server,
      request.action,
      request.scope.as_deref().unwrap_or_default()
    )
  };
  Column::new()
    .id(format!("{}-command", answering.id))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .background(ExtraColor::CodeBackground.role())
    .rounded(RadiusSize::Md)
    .child(
      Text::new(&words)
        .id(format!("{}-action", answering.id))
        .variant(CardsText::Command.role())
        .selectable(true)
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

fn meta(ctx: &mut Ctx, place: &Place, answering: &Answering<'_>, question: &str) -> Row {
  let id = answering.id;
  let request = answering.request;
  let clipboard = ctx.use_context::<Clipboard>().unwrap_or_default();
  let copied = format!(
    "{} · {}\n{}\n\n{question}",
    request.server,
    request.action,
    request.scope.as_deref().unwrap_or_default()
  );
  let copy = pressable(move || {
    if !clipboard.copy(&copied) {
      tracing::warn!("the approval card's text could not be copied to the clipboard");
    }
  })
  .id(format!("{id}-copy"))
  .describe("a11y_name", ctx.t("common.copy"))
  .child(icon(Icon::Copy, IconSize::Base, PaletteColor::TextMuted));
  let reply = Place::composer(ctx).map(|composer| {
    pressable(move || composer.take_up())
      .id(format!("{id}-reply"))
      .describe("a11y_name", ctx.t("cards.reply"))
      .child(icon(Icon::MessageSquareReply, IconSize::Base, PaletteColor::TextMuted))
  });
  let step = request::step(place, answering.shown).unwrap_or_default();
  let time = display::clock(request.requested_at);
  let words = ctx.t_args(
    "cards.question.meta",
    [
      ("agent", request.agent.as_str()),
      ("step", &*step),
      ("time", time.as_str()),
    ],
  );
  Row::new()
    .id(format!("{id}-meta"))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(copy)
    .with_children(reply)
    .child(Text::new(&words).variant(CardsText::Quiet.role()).nowrap())
}
