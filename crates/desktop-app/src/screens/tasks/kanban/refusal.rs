//! A column's refusal of a card, at its top (`Refusal` `q2Rtvh` in `03 Drop refused` `ZaM50`, and the refusals of `12
//! Moves and confirmations` `aUNO8`): over the column the drag is on, and after the drop, `surface_raised` in a
//! `danger` stroke, radius 6, with *KON-16 can't move to In review* (12/600 after a 13 `danger` mark), why in 12
//! `text_secondary`, and, for a run that has not ended, that Done or Canceled stop it after a confirmation in 12
//! `text_muted`. While a card is lifted, every other column that would refuse it says why in a quiet note at its top
//! (`Refused` `QMdRC`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::moves;

use super::actions::{Refusal, Refused};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The i18n key of why a column refuses a task, for a menu row that cannot be chosen.
pub const fn reason_key(refusal: moves::Refusal) -> &'static str {
  match refusal {
    moves::Refusal::RunNotEnded => "tasks.board.refused.runNotEndedMenu",
    moves::Refusal::Incomplete => "tasks.board.refused.incomplete",
    moves::Refusal::HasRun => "tasks.board.refused.hasRun",
  }
}

const fn words_key(refusal: &Refusal) -> &'static str {
  match refusal {
    Refusal::RunNotEnded => "tasks.board.refused.runNotEnded",
    Refusal::Incomplete(_) => "tasks.board.refused.incomplete",
    Refusal::HasRun => "tasks.board.refused.hasRun",
    Refusal::Pipeline => "tasks.board.refused.pipeline",
  }
}

/// The refusal of `refused` by the column `column` names, at its top.
pub fn refusal_card(ctx: &Ctx, refused: &Refused, column: &str) -> Column {
  let key = &*refused.key;
  let title = match &refused.refusal {
    Refusal::Incomplete(_) => ctx.t_args("tasks.board.refused.queueTitle", [("task", key)]),
    Refusal::HasRun => ctx.t_args("tasks.board.refused.backlogTitle", [("task", key)]),
    Refusal::RunNotEnded | Refusal::Pipeline => {
      ctx.t_args("tasks.board.refused.title", [("task", key), ("column", column)])
    }
  };
  let detail = match &refused.refusal {
    Refusal::Incomplete(missing) => Some((missing.clone(), PaletteColor::TextSecondary)),
    Refusal::RunNotEnded => Some((ctx.t("tasks.board.refused.closeHint"), PaletteColor::TextMuted)),
    Refusal::HasRun | Refusal::Pipeline => None,
  };
  Column::new()
    .id("tasks-board-refusal")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .bottom(ExtraSpacing::Loose.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Start)
        .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
        .child(
          Text::new(&title)
            .id("tasks-board-refusal-title")
            .variant(ExtraTypography::InspectorSection.role())
            .flex(1.0),
        ),
    )
    .child(
      Text::new(&ctx.t(words_key(&refused.refusal)))
        .id("tasks-board-refusal-text")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(detail.map(|(words, color)| {
      Text::new(&words)
        .id("tasks-board-refusal-detail")
        .variant(TypographyStyle::Description)
        .color(color)
        .width(Dimension::Pct(100.0))
    }))
}

/// Why a column the drag is not over would refuse the lifted card: a quiet note at its top.
pub fn refusal_note(ctx: &Ctx, refusal: &Refusal, id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::Ban, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(words_key(refusal)))
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
}
