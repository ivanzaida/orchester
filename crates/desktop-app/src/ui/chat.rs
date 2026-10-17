//! The messages of a conversation, a task's and the project's Orchestrator chat alike. An agent's (`Agent message`
//! `dcd0m`): a 24 avatar (`surface_raised` in a `border` stroke, radius 6, a 13 glyph in `text_secondary`), 10
//! from who wrote it (12/600) with what it was and when (11 `text_muted`) and under them, 3 apart, what it wrote.
//! A person's (`Your message` `mr4lC`): at the right, *You* and the time, the message as written in a bubble that
//! hugs it up to 80 % of the chat (`own_message` in a `border` stroke, radius 10, 8 by 12 inside) and, under it,
//! where it stands. Where a link in an Orchestrator's reply leads is [`links`]'; which places an agent names as
//! plain text are links, [`references`]'; which of its lists are labelled links, [`labelled`]'s.

pub mod labelled;
pub mod links;
pub mod presentation;
pub mod references;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::display;

use super::{
  icon::{Icon, icon},
  markdown::{LinkHandler, MarkdownView},
};
use crate::theme::{ConversationText, ExtraColor, ExtraSpacing, IconSize};

/// The avatar's box (`HO4bJ`) and how wide a person's bubble may grow, of the chat's width.
const AVATAR_SIZE: f32 = 24.0;
const BUBBLE_WIDTH: f32 = 80.0;

/// What a press on a link in a conversation does: the window's one handler ([`links::ChatLinks`]), which follows
/// only places in a project and pages on the web, and never opens the browser for synthetic input.
pub fn links(ctx: &mut Ctx) -> LinkHandler {
  links::ChatLinks::of(ctx)
}

/// Who a row is from, as its avatar shows it: an agent of a run, a command tool that runs a step, or the
/// project's Orchestrator (`waypoints`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Avatar {
  Agent,
  Tool,
  Orchestrator,
}

impl Avatar {
  const fn glyph(self) -> Icon {
    match self {
      Self::Agent => Icon::Bot,
      Self::Tool => Icon::Terminal,
      Self::Orchestrator => Icon::Waypoints,
    }
  }
}

/// The avatar, then `name` and `meta` with `body` under them; `id` names the row.
pub fn chat_row(id: String, avatar: Avatar, (name, meta): (&str, &str), body: Vec<Element>) -> Element {
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(avatar.glyph(), IconSize::Base, PaletteColor::TextSecondary)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Row::new()
            .spacing(ExtraSpacing::Snug.role())
            .align_items(Alignment::Center)
            .child(
              Text::new(name)
                .id("conversation-message-name")
                .variant(ConversationText::Name.role())
                .nowrap(),
            )
            .child(
              Text::new(meta)
                .id("conversation-message-meta")
                .variant(TypographyStyle::Caption)
                .nowrap(),
            ),
        )
        .with_children(body),
    )
    .into()
}

/// What an agent's message in a chat of `project` says, its Markdown rendered with the places it names as text
/// linked ([`references`]); `id` names it.
pub fn text(ctx: &mut Ctx, id: &str, (text, project): (&Arc<str>, ProjectId)) -> Element {
  let links = links(ctx);
  let shown = references::shown(ctx, text, project);
  MarkdownView::new(id, shown, links).render(ctx)
}

/// *You*, the time when it is known, the bubble with `body` as written, and `delivery` under it; `id` names the
/// message.
pub fn person_message(
  ctx: &mut Ctx,
  id: &str,
  (body, at): (&Arc<str>, Option<Timestamp>),
  delivery: Option<Element>,
) -> Column {
  let head = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("conversation.you"))
        .variant(TypographyStyle::FieldLabel)
        .nowrap(),
    )
    .with_children(at.map(|at| {
      Text::new(&display::clock(at))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }));
  // As the person wrote it, so the bubble hugs its words up to its widest (`pCzqz`): lurq's Markdown always
  // fills its parent's width.
  let said = Text::new(body)
    .id(format!("{id}-text"))
    .variant(TypographyStyle::Body)
    .selectable(true);
  let bubble = Column::new()
    .id(format!("{id}-bubble"))
    .max_width(Dimension::Pct(BUBBLE_WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .background(ExtraColor::OwnMessage.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(said);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::End)
    .child(head)
    .child(bubble)
    .with_children(delivery)
}

/// The line under a person's message: a 12 icon in `color` and the words in 11 `text_muted` (in `color` when it is
/// `text_primary`), 5 apart; `id` names it.
pub fn delivery_line(id: &str, (glyph, color): (Icon, PaletteColor), words: &str) -> Element {
  let strong = color == PaletteColor::TextPrimary;
  let words = Text::new(words).variant(TypographyStyle::Caption);
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(if strong { words.color(color) } else { words })
    .into()
}
