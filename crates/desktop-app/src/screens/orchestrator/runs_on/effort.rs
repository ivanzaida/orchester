//! The runs-on picker's Effort column (`Effort` `Js9xG` in `v6mWp8`): 196 wide, 8 over and under and 6 at its sides,
//! 2 apart, a `border` stroke on its left. Its 26-high head says *Effort* (12 `text_muted`) and the chosen model (Geist
//! Mono 10.5 `text_muted`); then *Default* with *the CLI's own* (11 `text_muted`) and the efforts the connection lists
//! for the model (12), the chosen one checked. A connection that lists no efforts (an API provider) has no column.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::ModelId;

use super::{item::item, picking::Chooser};
use crate::{
  screens::orchestrator::settings::effort_word,
  theme::{ExtraSpacing, OrchestratorText},
};

/// The column's width (`Js9xG`) and its head's height (`MvASh`).
const WIDTH: f32 = 196.0;
const HEAD_HEIGHT: f32 = 26.0;

/// The column for `model`, of `efforts` (`None` the CLI's own), `chosen` checked and the keys at `cursor`.
pub(super) fn column(
  ctx: &Ctx,
  (model, efforts): (&ModelId, &[Option<String>]),
  (chosen, cursor): (Option<&str>, Option<usize>),
  chooser: &Chooser,
) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.effort"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .nowrap(),
    )
    .child(
      Text::new(model.as_str())
        .id("orchestrator-runs-on-effort-model")
        .variant(OrchestratorText::PlanUse.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0)),
    );
  let rows = efforts.iter().enumerate().map(|(index, effort)| {
    let parts: Vec<Element> = match effort {
      None => vec![
        label(&ctx.t("orchestrator.runsOn.effortDefault"), false),
        Text::new(&ctx.t("orchestrator.runsOn.effortDefaultDetail"))
          .variant(TypographyStyle::Caption)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .into(),
      ],
      Some(effort) => vec![label(&effort_word(ctx, effort), true)],
    };
    let (chooser, effort) = (chooser.clone(), effort.clone());
    item(
      format!("orchestrator-runs-on-effort-{index}"),
      parts,
      (effort.as_deref() == chosen, cursor == Some(index)),
      move || chooser.effort(effort.clone()),
    )
  });
  Column::new()
    .id("orchestrator-runs-on-effort")
    .width(Dimension::Px(WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Tight.role())
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(head)
    .with_children(rows)
    .into()
}

/// An effort's words, filling the row when `fills`.
fn label(words: &str, fills: bool) -> Element {
  let text = Text::new(words)
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextPrimary)
    .nowrap();
  if fills {
    text.flex(1.0).min_width(Dimension::Px(0.0)).into()
  } else {
    text.into()
  }
}
