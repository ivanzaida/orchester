//! A chat row of the conversation (`kpnmP`): a 28 round avatar, then who wrote it (12/500) with its
//! meta (11 `$subtle`) on one line, and what the row says under it: the text at 13 / 1.6, read as
//! Markdown, a card, the chips of an agent's tool activity, a person's delivery line.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    markdown::MarkdownText,
  },
};

/// The avatar's box (`qU0XW`).
const AVATAR_SIZE: f32 = 28.0;

/// Who a row is from, as its avatar shows: an agent, the person, an agent's question or plan, a tool
/// step, or a tool request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Avatar {
  Agent,
  Person,
  Question,
  Plan,
  Tool,
  Request,
}

impl Avatar {
  fn look(self) -> (Icon, PaletteColor) {
    match self {
      Self::Agent => (Icon::Bot, PaletteColor::SurfaceRaised),
      Self::Person => (Icon::User, PaletteColor::AccentMuted),
      Self::Question => (Icon::MessageCircleQuestion, PaletteColor::SurfaceRaised),
      Self::Plan => (Icon::ListChecks, PaletteColor::SurfaceRaised),
      Self::Tool => (Icon::Terminal, PaletteColor::SurfaceRaised),
      Self::Request => (Icon::Hand, PaletteColor::SurfaceRaised),
    }
  }
}

/// The row `id`: `avatar`, `name` and `meta`, then `body`.
pub(super) fn chat_row(id: String, avatar: Avatar, (name, meta): (&str, &str), body: Vec<Element>) -> Element {
  let (glyph, background) = avatar.look();
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
        .flex_shrink(0.0)
        .background(background)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(ExtraRadius::Round.role())
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(glyph, IconSize::Event, PaletteColor::TextPrimary)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(
              Text::new(name)
                .id("conversation-row-name")
                .variant(ExtraTypography::NoticeHeading.role())
                .nowrap(),
            )
            .child(
              Text::new(meta)
                .id("conversation-row-meta")
                .variant(TypographyStyle::Caption)
                .color(PaletteColor::TextMuted)
                .nowrap(),
            ),
        )
        .with_children(body),
    )
    .into()
}

/// What a message says (`V17jfc`): 13 at 1.6, wrapping, selectable, its Markdown rendered; `id` names
/// it among the parent's children.
pub(super) fn chat_text(ctx: &mut Ctx, id: &str, text: &Arc<str>) -> Element {
  MarkdownText::message(id, text.clone()).mount(ctx)
}
