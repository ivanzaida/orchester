//! What grouped rows (`Published` `pnnmh` in `x1lmXb`, `Step` `E3MuZQ` in `IsU5u`) put beside a label and under a
//! group: a tag that says *Default* or *Changed*, and a note.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::dimension::Dimension,
  node::padding::Padding,
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraColor, ExtraSpacing, GroupText, IconSize};

/// A tag beside a label: *Default* in a round `surface_raised` chip (`i3mZr`), or *Changed* in `accent_text` in a
/// round `selection` outline (`X4LDh`).
pub fn tag(id: &str, word: &str, changed: bool) -> Row {
  let row = Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .rounded(crate::theme::ExtraRadius::Full.role());
  if changed {
    row
      .border_inside(BorderSize::Sm, ExtraColor::Selection.role())
      .child(icon(Icon::FilePen, IconSize::Xs, ExtraColor::AccentText.role()))
      .child(Text::new(word).variant(GroupText::Tag.role()).nowrap())
  } else {
    row
      .background(PaletteColor::Card)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .child(
        Text::new(word)
          .variant(TypographyStyle::Caption)
          .color(ExtraColor::TextSecondary.role())
          .nowrap(),
      )
  }
}

/// A note under a group: an `info` glyph and its words in 12 `text_muted` (`m7d9w9`).
pub fn note(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::MutedForeground))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(GroupText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
