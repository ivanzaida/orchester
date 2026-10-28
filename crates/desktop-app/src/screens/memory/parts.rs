//! Small parts the Memory screens share: a group's heading (`Memory / Group heading` `CJvK7`), a record's tags and
//! its latest-handoff flag, the glyph column, and a status label of a record no agent is given.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, memory::Memory, project::TaskKey};

use crate::{
  navigation::{Navigation, link::Open},
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraRadius, ExtraSpacing, IconSize, MemoryText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The glyph column of a record row: 16 wide, a line of its text high (`k1oxYF` in `J1utD`).
const GLYPH_WIDTH: f32 = 16.0;
const GLYPH_HEIGHT: f32 = 19.0;

/// A group's heading: its label in capitals and its count (`CJvK7`).
pub fn group_heading(id: &str, label: &str, count: usize) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(label).variant(MemoryText::GroupLabel.role()).nowrap())
    .child(
      Text::new(&count.to_string())
        .id(format!("{id}-count"))
        .variant(MemoryText::GroupCount.role())
        .nowrap(),
    )
}

/// A record type's glyph in its column (`W0R4Mf`, 14 `text_muted`).
pub fn glyph(mark: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_WIDTH))
    .height(Dimension::Px(GLYPH_HEIGHT))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(mark, IconSize::Md, PaletteColor::TextMuted))
}

/// `memory`'s tags (`iRiGk` in `J1utD`).
pub fn tags(id: &str, memory: &Memory) -> Option<Row> {
  let tags = memory.content().tags();
  tags.iter().next()?;
  Some(
    Row::new()
      .id(format!("{id}-tags"))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .with_children(
        tags
          .iter()
          .map(|tag| TagChip::new(format!("{id}-tag-{tag}"), tag.to_owned())),
      ),
  )
}

/// The latest handoff's flag, *Latest · given to every agent* (`EJNvR` in `J1utD`).
pub fn flag(id: &str, label: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .child(Text::new(label).variant(MemoryText::Flag.role()).nowrap())
}

/// A status label: a 12 glyph and its words, 5 apart (`b2J5Cr` in `J1utD`, `V6r5PT` in `LdHs2`).
pub fn status(id: &str, (mark, words): (Icon, Arc<str>), color: PaletteColor) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .flex_shrink(0.0)
    .child(icon(
      mark,
      IconSize::Sm,
      if color == PaletteColor::TextSecondary {
        PaletteColor::TextMuted
      } else {
        color.clone()
      },
    ))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::FieldLabel)
        .color(color)
        .nowrap(),
    )
}

/// A link under a panel or a fact: its words in `accent_text`, opening `open` (`RkGXO`, `I7UAFO`).
pub fn link(id: &str, words: &str, open: Open) -> Element {
  opening(open)
    .id(id.to_owned())
    .describe("a11y_name", words.to_owned())
    .child(Text::new(words).variant(MemoryText::Link.role()).nowrap())
    .into()
}

/// A link to the conversation of task `task` of `project` (*Show in Conversation ↗*).
pub fn conversation_link(ctx: &mut Ctx, (project, task): (ProjectId, &TaskKey), (id, words): (&str, &str)) -> Element {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(task.clone(), TaskView::Conversation));
  link(id, words, navigation.to(route))
}

/// *Conversation ↗* at a source row's end, opening the conversation of task `task` of `project` (`i14UW` in 05c,
/// `X71XlI` in 05e).
pub fn conversation_button(ctx: &mut Ctx, (project, task): (ProjectId, &TaskKey), id: &str) -> Element {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(task.clone(), TaskView::Conversation));
  ActionButton::new(id, ButtonKind::Ghost, ctx.t("memory.inspector.conversation"))
    .icon(Icon::ArrowUpRight)
    .opens(navigation.to(route))
    .into()
}

/// What a link to a page not built yet says, in place of the link (*Project settings › Memory* today).
pub fn unbuilt_link(id: &str, words: &str, reason: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(MemoryText::Link.role())
    .describe("state", "unavailable")
    .describe("reason", reason.to_owned())
    .opacity(crate::theme::DISABLED_OPACITY)
    .into()
}
