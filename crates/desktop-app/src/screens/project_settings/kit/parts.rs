//! The other patterns of the page: an empty row (`Empty · No sources yet` `JKOnV`: an outlined 28 tile, what is
//! missing in 13/500 `text_secondary` over one sentence in 12 `text_muted`, and its action), the question asked in
//! place (`Archive · question` `fw0eT`: `surface_raised` inside a `border` stroke, radius 6, 10 over and under and 12 at
//! its sides, its title in 13/600 over one sentence in 12 `text_secondary` and its actions, 8 apart), a sub-page's back
//! crumb (`Crumb` `YYkW6`: an arrow and where it goes back to in 12 `text_muted`, the page after a `/` in
//! `text_secondary`), a sub-page's head (`Head` `mHwXV`: its title in 16/600 over what it does in 12.5
//! `text_secondary`), an opened item's head (`Head` `a4Inip`: a 36 tile, its name in 16/600 over its status, its
//! actions at the end), an item's 28 tile (`Tile` `oxMxA`) and a terminal command with Copy (`Command` `SiCGv`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::row::Tone;
use crate::{
  navigation::link::Open,
  theme::{
    ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsSpacing, ProjectSettingsText, SettingsText,
  },
  ui::{
    code_box::CodeBox,
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// An item's tile (`Tile` `oxMxA`).
const TILE: f32 = 28.0;
/// An opened item's tile (`Tile` `AFO61`).
const HEAD_TILE: f32 = 36.0;

/// An item's 28 tile on `surface_raised` inside a `border` stroke, radius 6, its 14 glyph `text_secondary`.
pub(crate) fn tile(glyph: Icon) -> Row {
  square(TILE, RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
}

/// An empty row's 28 tile: only its `border` stroke, its 14 glyph `text_muted` (`Tile` `Uagnp`).
pub(crate) fn outlined_tile(glyph: Icon) -> Row {
  square(TILE, RadiusSize::Md).child(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
}

fn square(size: f32, radius: RadiusSize) -> Row {
  Row::new()
    .width(Dimension::Px(size))
    .height(Dimension::Px(size))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(radius)
}

/// A group's row for what it has none of yet (`Empty · No MCP servers yet` `hJSDK`).
pub(crate) struct EmptyRow {
  id: Arc<str>,
  glyph: Icon,
  title: Arc<str>,
  detail: Arc<str>,
  actions: Vec<Element>,
}

impl EmptyRow {
  /// The row `id` (its title `<id>-title`, its sentence `<id>-detail`).
  pub(crate) fn new(id: &str, glyph: Icon, title: impl Into<Arc<str>>, detail: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      glyph,
      title: title.into(),
      detail: detail.into(),
      actions: Vec::new(),
    }
  }

  pub(crate) fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }
}

impl From<EmptyRow> for Element {
  fn from(row: EmptyRow) -> Self {
    let id = row.id;
    Row::new()
      .id(id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ProjectSettingsSpacing::RowX.role(), SpacingSize::Md))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(square(TILE, RadiusSize::Md).child(icon(row.glyph, IconSize::Md, PaletteColor::TextMuted)))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&row.title)
              .id(format!("{id}-title"))
              .variant(ExtraTypography::Strong.role())
              .color(Tone::Secondary.color()),
          )
          .child(
            Text::new(&row.detail)
              .id(format!("{id}-detail"))
              .variant(ProjectSettingsText::Hint.role())
              .width(Dimension::Pct(100.0)),
          ),
      )
      .with_children((!row.actions.is_empty()).then(|| {
        Row::new()
          .flex_shrink(0.0)
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .with_children(row.actions)
      }))
      .into()
  }
}

/// The question asked in place before a destructive or widening change: `title`, what it does (a widening says each
/// thing it widens on a line of its own), and `actions`, the action first and *Keep it* after it.
pub(crate) fn question(id: &str, title: &str, body: &[Arc<str>], actions: Vec<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(SettingsText::SectionTitle.role()),
    )
    .with_children(body.iter().enumerate().map(|(index, line)| {
      let text = Text::new(line)
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0));
      if index == 0 {
        text.id(format!("{id}-body"))
      } else {
        text
      }
    }))
    .child(
      Row::new()
        .id(format!("{id}-actions"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(actions),
    )
}

/// The crumb back to `parent`, which `open` opens, with the page itself after it when it has a name of its own.
pub(crate) fn back(id: &str, (parent, here): (&str, Option<&str>), open: Open) -> Element {
  let link = opening(open)
    .id(format!("{id}-back"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(crate::theme::ExtraColor::Hover.role()))
    .describe("a11y_name", parent.to_owned())
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(parent)
        .variant(ProjectSettingsText::Hint.role())
        .nowrap(),
    );
  let here = here.map(|here| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(Text::new("/").variant(ProjectSettingsText::Hint.role()))
      .child(
        Text::new(here)
          .id(format!("{id}-here"))
          .variant(ProjectSettingsText::Hint.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
  });
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(link)
    .with_children(here)
    .into()
}

/// A sub-page's title over what it does, if it says.
pub(crate) fn page_head(id: &str, title: &str, lead: Option<&str>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ProjectSettingsText::PageTitle.role()),
    )
    .with_children(lead.map(|lead| {
      Text::new(lead)
        .id(format!("{id}-lead"))
        .variant(ExtraTypography::Lead.role())
        .width(Dimension::Pct(100.0))
    }))
    .into()
}

/// An opened item's head: its 36 tile with `glyph`, `title` over `status`, and `actions` at the end.
pub(crate) fn entity_head(id: &str, (glyph, title): (Icon, Element), status: Element, actions: Vec<Element>) -> Element {
  let tile = square(HEAD_TILE, ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextSecondary));
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(title)
        .child(status),
    )
    .with_children(actions)
    .into()
}

/// A command to run in a terminal, with Copy, under the line that says why.
pub(crate) fn command_line(ctx: &mut Ctx, id: &str, label: Arc<str>, command: impl Into<Arc<str>>) -> Element {
  CodeBox::new(id.to_owned(), label, command).render(ctx)
}
