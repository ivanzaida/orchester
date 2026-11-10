//! A row of the Agents hub's list, one for a team and a local project (`Item · Builder` `tF2OL`, `Item · pencil-design`
//! `U9RFS`, L2): the kind's glyph, the name over its line, and at its end a tag and a version. Every row is the same
//! height whatever its kind or how it stands, so the list never changes its rhythm between tabs. The chosen row stands
//! inert on `hover` with a 2 `accent` start (`n330IO`); the others open the hub on them. What the glyph, the line, the
//! tag and the version say is each hub's own; how the row is laid out is this.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Button, Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::readiness::Group;

use crate::{
  theme::{ExtraColor, ExtraSpacing, HubText},
  ui::pressable::inert,
};

/// A row's height (`Item · Builder` `tF2OL`): the name over its line, the same for every row.
pub(crate) const ROW_HEIGHT: f32 = 56.0;
/// A tag's height (`Tag` `uJ2rP`).
const TAG_HEIGHT: f32 = 20.0;

/// What a row shows. `open` is what a press on a row that is not chosen does.
pub(crate) struct AgentRow<'a> {
  pub id: &'a str,
  pub chosen: bool,
  pub open: Button,
  pub group: Group,
  /// What the row is, for tools reading the tree.
  pub kind: String,
  pub mark: Element,
  pub name: &'a str,
  /// The line under the name: [`line`] for one that is cut without a word, or text that shows its full value when cut.
  pub line: Element,
  pub tag: Option<Row>,
  /// The version at its end, already worded (*v4*).
  pub version: Option<String>,
}

pub(crate) fn agent_row(row: AgentRow<'_>) -> Element {
  let id = row.id;
  let button = if row.chosen { inert(Button::empty()) } else { row.open };
  let start = if row.chosen {
    ExtraSpacing::Inset.role()
  } else {
    SpacingSize::Lg
  };
  let button = button
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::new().left(start).right(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("state", if row.chosen { "chosen" } else { "enabled" })
    .describe("group", group_name(row.group))
    .describe("kind", row.kind)
    .child(row.mark)
    .child(text(id, row.name, row.line))
    .with_children(row.tag)
    .with_children(row.version.map(|version| {
      Text::new(&version)
        .id(format!("{id}-version"))
        .variant(HubText::RowLine.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
    }));
  if row.chosen {
    button
      .background(ExtraColor::Hover.role())
      .border_left(Border::inside(BorderSize::Md, PaletteColor::Accent))
      .into()
  } else {
    button
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// The name over its line (`Text` in `U9RFS`).
fn text(id: &str, name: &str, line: Element) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(name)
        .id(format!("{id}-name"))
        .variant(HubText::RowName.role())
        .nowrap(),
    )
    .child(line)
}

/// The line under a name, cut with an ellipsis where its row runs out of room (`Text` in `U9RFS`).
pub(crate) fn line(id: &str, words: &str, color: PaletteColor) -> Text {
  Text::new(words)
    .id(format!("{id}-line"))
    .variant(HubText::RowLine.role())
    .color(color)
    .width(Dimension::Pct(100.0))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
}

/// A tag (`Tag` `uJ2rP`).
pub(crate) fn tag_chip(id: &str, words: &str) -> Row {
  Row::new()
    .id(id)
    .height(Dimension::Px(TAG_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(
      Text::new(words)
        .id(format!("{id}-label"))
        .variant(HubText::RowTag.role())
        .nowrap(),
    )
}

const fn group_name(group: Group) -> &'static str {
  match group {
    Group::NeedsYouHere => "needs",
    Group::Ready => "ready",
    Group::OffHere => "off",
  }
}
