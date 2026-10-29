//! A tool request auto mode's reviewer is still judging: it waits for the reviewer, not for the person, so it has no
//! answers and the run reads as at work. A quiet row in the decided row's anatomy ([`super::decided`]): *Auto mode is
//! checking this command…*, what was asked on one line in mono, and *Answer myself*, which opens the whole card while
//! the review goes on (no frame draws it). Once the reviewer approves, the row is the decided *Approved by auto mode
//! at …*; once it leaves the request to the person (it held or denied it, failed, or ran past the runner's limit),
//! the whole card shows.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::PendingToolApproval;

use super::decided::asked;
use crate::{
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The row of `request` under review; *Answer myself* sets `answer_myself`.
pub(super) fn row(ctx: &Ctx, id: &str, request: &PendingToolApproval, answer_myself: &Signal<bool>) -> Element {
  let title = ctx.t(if request.command {
    "cards.approval.checkingCommand"
  } else {
    "cards.approval.checkingTool"
  });
  let open = answer_myself.clone();
  let answer = ActionButton::new(
    format!("{id}-answer-myself"),
    ButtonKind::Ghost,
    ctx.t("cards.approval.answerMyself"),
  )
  .small()
  .on_click(move || open.set(true));
  Row::new()
    .id(id.to_owned())
    .describe("waiting", "false")
    .describe("reviewing", "true")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::Bot, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id(format!("{id}-checking"))
            .variant(CardsText::Decided.role()),
        )
        .child(
          Text::new(&asked(ctx, request))
            .id(format!("{id}-asked"))
            .variant(ExtraTypography::MonoKey.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(answer)
    .into()
}
