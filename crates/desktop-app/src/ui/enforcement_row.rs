//! Enforcement row (`Enforcement row` `CR9n5`): what agents may do, its state at the end of the line (a 12
//! `shield-check` and *Enforced* in 11.5/500 `success`, 4 apart, or a 12 `info` in `text_muted` without a word), the
//! rule in 12 `text_secondary` and what holds it in 11.5/1.4 `text_muted`, 3 apart, 10 over and under, a `border`
//! line under it unless it is the last.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// Whether Orchester holds the rule itself, or it only says where it is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
  /// Enforced, in these words.
  Enforced,
  /// Set elsewhere: an `info` mark and no word.
  Elsewhere,
}

/// The row `id`: `setting`, its `rule` and what holds it, `mechanism`; `enforced` is the state's word.
pub fn enforcement_row(
  id: &str,
  (setting, rule, mechanism): (&str, &str, &str),
  held: Held,
  enforced: &str,
  last: bool,
) -> Element {
  let state = match held {
    Held::Enforced => Row::new()
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::Success))
      .child(
        Text::new(enforced)
          .id(format!("{id}-state"))
          .variant(ExtraTypography::NoteStrong.role())
          .color(PaletteColor::Success),
      ),
    Held::Elsewhere => Row::new().child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted)),
  };
  let row = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(setting)
            .id(format!("{id}-setting"))
            .variant(TypographyStyle::Label)
            .flex(1.0),
        )
        .child(state),
    )
    .child(
      Text::new(rule)
        .id(format!("{id}-rule"))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(mechanism)
        .id(format!("{id}-mechanism"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  if last {
    return row.into();
  }
  row
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .into()
}
