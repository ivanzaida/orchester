//! A fenced code block of agent-written Markdown, as plan review's code blocks read (`theme::markdown`:
//! `code-bg` with the `border` stroke, radius-control, 12 padding, its language 8 above its lines in
//! `subtle`), its lines in Geist Mono as written, never wrapped: a line wider than the text scrolls
//! sideways inside the block, with the transcript command's thumb over its bottom padding.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, ScrollHorizontal, Text},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, command_scrollbar};

/// The text styles of a code block: its language's, the body's, and its lines'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CodeStyle {
  pub(super) label: TypographyStyle,
  pub(super) code: TypographyStyle,
}

/// The block `id`, with `language` above `code` when the fence named one.
pub(super) fn code_block(id: String, style: CodeStyle, (language, code): (Option<&str>, &str)) -> Element {
  let label = language.map(|language| {
    Text::new(language)
      .variant(style.label)
      .color(PaletteColor::TextMuted)
      .nowrap()
  });
  let scroll = format!("{id}-scroll");
  let lines = Text::new(code)
    .id(format!("{id}-text"))
    .variant(style.code)
    .color(ExtraColor::CodeText.role())
    .selectable(true)
    .nowrap();
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(label)
    .child(
      ScrollHorizontal::new(Column::new().padding_bottom(ExtraSpacing::Stack.role()).child(lines))
        .id(scroll)
        .scrollbar(command_scrollbar())
        .width(Dimension::Pct(100.0))
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
