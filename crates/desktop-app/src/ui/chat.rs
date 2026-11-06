//! The messages of a conversation, a task's and the project's Orchestrator chat alike. An agent's (`Agent message`
//! `dcd0m`): a 24 avatar (`surface_raised` in a `border` stroke, radius 6, a 13 glyph in `text_secondary`), 10
//! from who wrote it (12/600) with what it was and when (11 `text_muted`) and under them, 3 apart, what it wrote
//! ([`agent_text`]). A person's (`Your message` `mr4lC`): at the right, *You* and the time, the message as written in
//! a bubble that hugs it up to 80 % of the chat (`own_message` in a `border` stroke, radius 10, 8 by 12 inside) and,
//! under it, where it stands. Where a link in what an agent wrote leads is [`links`]'; which places an agent names as
//! plain text are links, [`references`]'; which of its lists are labelled links, [`labelled`]'s. Both chats open at
//! their end and follow it ([`follow`]); what a search found in a message is filled behind its words ([`found`]).
//! A message and a code block in it can be copied ([`copy`], [`code_block`]); a message's actions can offer *Edit*,
//! *Rewind to here* and *Fork from here* besides ([`toolbar`]).

mod agent_text;
pub mod code_block;
pub mod copy;
mod copyable;
pub mod export;
pub mod fences;
pub mod follow;
pub mod found;
mod grid;
pub mod labelled;
pub mod links;
pub mod names;
mod place;
pub mod presentation;
pub mod references;
pub mod said;
pub mod toolbar;

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
use orchester_core::time::Timestamp;
use orchester_workspace::display;

use self::found::Found;
pub use self::{
  agent_text::agent_text,
  copyable::{row as copyable_row, row_with as copyable_row_with},
};
use super::{
  icon::{Icon, icon},
  markdown::LinkHandler,
};
use crate::theme::{ConversationText, ExtraColor, ExtraSpacing, IconSize};

/// What a person's message says: the element that shows it, and their words as written, which its Copy puts on the
/// clipboard.
pub type Said = (Element, Arc<str>);

/// The avatar's box (`HO4bJ`) and how wide a person's bubble may grow, of the chat's width.
pub const AVATAR_SIZE: f32 = 24.0;
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
  chat_row_marked(id, avatar, (name, meta, None), body)
}

/// [`chat_row`] with `mark` on the name line after `meta`: an ended answer's plan chip (09 g `OoT5k`).
pub fn chat_row_marked(
  id: String,
  avatar: Avatar,
  (name, meta, mark): (&str, &str, Option<Element>),
  body: Vec<Element>,
) -> Element {
  message_row(id, avatar, head((name, meta, mark)), body).into()
}

/// Who wrote a message, `name`, then `meta` and `mark`.
fn head((name, meta, mark): (&str, &str, Option<Element>)) -> Row {
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
    )
    .with_children(mark)
}

/// The avatar, then `head` with `body` under it; `id` names the row.
fn message_row(id: String, avatar: Avatar, head: Row, body: Vec<Element>) -> Row {
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
        .child(head)
        .with_children(body),
    )
}

/// *You*, the time when it is known, the bubble with what the message says (`said`, [`said::said`]) and its Copy, the
/// files attached to it (`attached`, [`super::attachments::sent`]) and `delivery` under it; `id` names the message. A
/// message of attachments only has no bubble.
pub fn person_message(
  ctx: &mut Ctx,
  id: &str,
  (said, at): (Option<Said>, Option<Timestamp>),
  (attached, delivery): (Option<Element>, Option<Element>),
) -> Column {
  let you = ctx.t("conversation.you");
  person_side_message(ctx, id, (&you, said, at), (attached, delivery))
}

/// A message on the person's side as [`person_message`] shows it, written by `author` (the project's Orchestrator
/// for one it sent the agents of a task) instead of *You*.
pub fn person_side_message(
  ctx: &mut Ctx,
  id: &str,
  (author, said, at): (&str, Option<Said>, Option<Timestamp>),
  (attached, delivery): (Option<Element>, Option<Element>),
) -> Column {
  let head = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(author)
        .id(format!("{id}-author"))
        .variant(TypographyStyle::FieldLabel)
        .nowrap(),
    )
    .with_children(at.map(|at| {
      Text::new(&display::clock(at))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }));
  message_column(ctx, id, Some(head), said, (attached, delivery))
}

/// The person's message without its *You* and time (`Your message` in the Orchestrator's `01 Conversation`
/// `wRSL4`): the bubble with what it says and its Copy, the files attached to it and `delivery` under it; `id` names
/// it.
pub fn own_bubble(
  ctx: &mut Ctx,
  id: &str,
  said: Option<Said>,
  (attached, delivery): (Option<Element>, Option<Element>),
) -> Column {
  message_column(ctx, id, None, said, (attached, delivery))
}

/// [`own_bubble`] whose actions hold `extras` around Copy ([`toolbar`]), in the line under the message, before
/// `delivery` (`06 · Message actions` `XPDAX`).
pub fn own_bubble_with(
  ctx: &mut Ctx,
  id: &str,
  said: Option<Said>,
  (attached, delivery): (Option<Element>, Element),
  extras: toolbar::MessageExtras,
) -> Column {
  let (bubble, words) = said.map_or((None, Arc::from("")), |(said, words)| {
    (Some(bubble_box(id, said).into()), words)
  });
  let attached = attached.map(|attached| attached_row(attached).into());
  let message = copyable::person(ctx, id, (&words, Some(extras)), [bubble, attached], Some(delivery));
  person_column(id).child(message)
}

/// What `body` says as written, without links (a note the person returned the work with), what a search found in it
/// filled behind its words (`found`); `id` names the message.
pub fn plain_said(ctx: &Ctx, id: &str, body: &str, found: Option<&Found>) -> Option<Said> {
  (!body.trim().is_empty()).then(|| {
    let words = Text::new(body)
      .id(format!("{id}-text"))
      .variant(TypographyStyle::Body)
      .selectable(true);
    (marked_words(ctx, words, body, found), Arc::from(body))
  })
}

/// `words`, a message's words that show `text` as written, with what a search found in them filled behind them.
pub fn marked_words(ctx: &Ctx, words: Text, text: &str, found: Option<&Found>) -> Element {
  let Some(found) = found else {
    return words.into();
  };
  let highlights = found.highlights(ctx, text);
  let places = highlights.len();
  found.classed(words.highlights(highlights).into(), places)
}

/// The bubble of the person's message `id` that holds `said`. Words as the person wrote them hug it up to its widest
/// (`pCzqz`); words with mentions are lurq's Markdown ([`said::said`]), which fills it.
fn bubble_box(id: &str, said: Element) -> Column {
  Column::new()
    .id(format!("{id}-bubble"))
    .max_width(Dimension::Pct(BUBBLE_WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .background(ExtraColor::OwnMessage.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(said)
}

/// The files of a message, at its end, as wide as the bubble may grow.
fn attached_row(attached: Element) -> Row {
  Row::new()
    .max_width(Dimension::Pct(BUBBLE_WIDTH))
    .justify(Justify::End)
    .child(attached)
}

/// The person's message `id`: `head`, then what it says in its bubble with the files attached to it, and the line
/// under them with Copy and `delivery` ([`copyable::person`]); a message of attachments only has neither bubble nor
/// Copy.
fn message_column(
  ctx: &mut Ctx,
  id: &str,
  head: Option<Row>,
  said: Option<Said>,
  (attached, delivery): (Option<Element>, Option<Element>),
) -> Column {
  let column = person_column(id).with_children(head);
  let attached = attached.map(attached_row);
  match said {
    Some((said, words)) => {
      let parts = [Some(bubble_box(id, said).into()), attached.map(Into::into)];
      column.child(copyable::person(ctx, id, (&words, None), parts, delivery))
    }
    None => column.with_children(attached).with_children(delivery),
  }
}

/// The column of the person's message `id`, at the chat's end.
fn person_column(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::End)
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
