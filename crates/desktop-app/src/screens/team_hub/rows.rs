//! The boxes of the panels' sections (`Rows` `tQ5pu`, `YTWxX`, `MH6ZR` in `Agents · Profile` `NDZXC`; `Rows` in `C4`
//! `SpRvd`): a row is its key (a label over an optional sub), 170 wide (150 in an MCP server's *On this computer*), and
//! its value, 12 apart, 8 over and under them and 12 at the sides; the rows stand in one `border` outline, radius `md`,
//! each ruled from the next.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  screens::team_hub::row::tag_chip,
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::chip::{BadgeTone, badge},
};

/// How wide a row's key is (`Key` `P60rze` in `NDZXC`, `Key` in `SpRvd` … `lSA4x`).
#[derive(Clone, Copy)]
pub(super) enum Key {
  Wide,
  /// The key of the rows of an MCP server's *On this computer*.
  Narrow,
}

impl Key {
  fn width(self) -> f32 {
    match self {
      Self::Wide => HubSpacing::Key.value(),
      Self::Narrow => HubSpacing::KeyNarrow.value(),
    }
  }
}

/// What a row's label says under it, and the color it says it in.
pub(super) struct Sub {
  pub words: Arc<str>,
  pub color: PaletteColor,
}

impl Sub {
  pub(super) const fn quiet(words: Arc<str>) -> Self {
    Self {
      words,
      color: PaletteColor::TextMuted,
    }
  }
}

/// A section: its head (the label, then what follows it on the line) over its rows.
pub(super) fn section(id: &str, label: &str, after: Vec<Element>, rows: Vec<Row>) -> Column {
  let last = rows.len().saturating_sub(1);
  let rows = rows.into_iter().enumerate().map(|(index, row)| {
    if index == last {
      row
    } else {
      row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(label)
            .id(format!("{id}-label"))
            .variant(HubText::Kind.role())
            .color(PaletteColor::TextMuted)
            .nowrap(),
        )
        .with_children(after),
    )
    .child(
      Column::new()
        .id(format!("{id}-rows"))
        .width(Dimension::Pct(100.0))
        .rounded(RadiusSize::Md)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .clip()
        .with_children(rows),
    )
}

/// One row: `label` over `sub`, and `value` beside them.
pub(super) fn row(id: &str, label: &str, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  row_in(Key::Wide, id, label, sub, value)
}

/// [`row`] whose key is `key` wide.
pub(super) fn row_in(key: Key, id: &str, label: &str, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  keyed_in(
    key,
    id,
    Text::new(label)
      .id(format!("{id}-label"))
      .variant(HubText::Status.role()),
    sub,
    value,
  )
}

/// [`row_in`] whose key is `label`, already a text: a name that is an identifier, in mono.
pub(super) fn keyed_in(key: Key, id: &str, label: Text, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  let key = Column::new()
    .width(Dimension::Px(key.width()))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(label)
    .with_children(sub.map(|sub| {
      Text::new(&sub.words)
        .id(format!("{id}-sub"))
        .variant(HubText::PanelSub.role())
        .color(sub.color)
    }));
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(key)
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .align_items(Alignment::Center)
        .child(value.into()),
    )
}

/// A value that is words.
pub(super) fn words(id: &str, words: &str) -> Text {
  Text::new(words).id(id.to_owned()).variant(HubText::Value.role())
}

/// A value that is a model's name or another identifier.
pub(super) fn mono(id: &str, words: &str) -> Text {
  Text::new(words).id(id.to_owned()).variant(HubText::PanelMono.role())
}

/// A value that is names, each in a tag; they wrap onto the next line when the row runs out of room.
pub(super) fn tags(id: &str, names: impl IntoIterator<Item = String>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      names
        .into_iter()
        .enumerate()
        .map(|(index, name)| tag_chip(&format!("{id}-{index}"), &name)),
    )
}

/// *On this computer* (`On this computer` `qtmpw` in `NDZXC`, `JYO4l` in `SpRvd`): what is only this computer's and
/// never sent to the team, as `rows`, with `after` under them.
pub(super) fn on_this_computer(ctx: &Ctx, rows: Vec<Row>, after: Option<Element>) -> Column {
  let note = Text::new(&ctx.t("team_hub.profile.onComputerNote"))
    .id("team-hub-on-computer-note")
    .variant(HubText::Hint.role())
    .nowrap();
  section(
    "team-hub-on-computer",
    &ctx.t("team_hub.profile.onComputer"),
    vec![
      badge("team-hub-yours", BadgeTone::Neutral, &ctx.t("team_hub.profile.yours")).into(),
      note.into(),
    ],
    rows,
  )
  .with_children(after)
}
