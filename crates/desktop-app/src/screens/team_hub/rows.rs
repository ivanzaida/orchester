//! The boxes of the panels' sections (`Rows` `tQ5pu`, `YTWxX`, `MH6ZR` in `Agents · Profile` `NDZXC`; `Rows` in `C4`
//! `SpRvd`): a row is its key (a label over an optional sub), 170 wide, and its value, 12 apart, 8 over and under them
//! and 12 at the sides; the rows stand in one `border` outline, radius `md`, each ruled from the next.

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
  screens::hub::agent_row::tag_chip,
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::chip::{BadgeTone, badge},
};

/// How wide a row's key is (`Key` `P60rze` in `NDZXC`, `Key` in `SpRvd` … `lSA4x`).
#[derive(Clone, Copy)]
pub(in crate::screens) enum Key {
  Wide,
  /// The key of a create form's rows.
  Form,
  /// [`Self::Wide`], whose label keeps one line and may run on into the gap before the value.
  OneLine,
}

impl Key {
  fn width(self) -> f32 {
    match self {
      Self::Wide | Self::OneLine => HubSpacing::Key.value(),
      Self::Form => HubSpacing::KeyForm.value(),
    }
  }
}

/// What a row's label says under it, and the color it says it in.
pub(in crate::screens) struct Sub {
  pub words: Arc<str>,
  pub color: PaletteColor,
}

impl Sub {
  pub(in crate::screens) const fn quiet(words: Arc<str>) -> Self {
    Self {
      words,
      color: PaletteColor::TextMuted,
    }
  }
}

/// A section: its head (the label, then what follows it on the line) over its rows.
pub(in crate::screens) fn section(id: &str, label: &str, after: Vec<Element>, rows: Vec<Row>) -> Column {
  let label = Text::new(label)
    .id(format!("{id}-label"))
    .variant(HubText::PanelLabel.role())
    .nowrap();
  section_headed(id, label.into(), after, rows)
}

/// [`section`] whose label is `label`, already drawn: a bone while the panel is read.
pub(in crate::screens) fn section_headed(id: &str, label: Element, after: Vec<Element>, rows: Vec<Row>) -> Column {
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
        .child(label)
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
pub(in crate::screens) fn row(id: &str, label: &str, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  row_in(Key::Wide, id, label, sub, value)
}

/// [`row`] whose key is `key` wide.
pub(in crate::screens) fn row_in(key: Key, id: &str, label: &str, sub: Option<Sub>, value: impl Into<Element>) -> Row {
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

/// [`row`] whose key is `label`, already drawn: a name that is an identifier, in mono, or a bone while the panel is
/// read.
pub(in crate::screens) fn keyed(
  id: &str,
  label: impl Into<Element>,
  sub: Option<Sub>,
  value: impl Into<Element>,
) -> Row {
  keyed_in(Key::Wide, id, label, sub, value)
}

/// [`row`] whose label keeps one line, running on past the key into the gap before the value where it is a little
/// longer than the key (*Ask when a tool needs approval*, `Jnrie` in `Pipelines · v2` P2b `EGEos`).
pub(in crate::screens) fn row_one_line(id: &str, label: &str, value: impl Into<Element>) -> Row {
  keyed_in(
    Key::OneLine,
    id,
    Text::new(label)
      .id(format!("{id}-label"))
      .variant(HubText::Status.role())
      .nowrap(),
    None,
    value,
  )
}

/// [`keyed`] whose key is `key` wide.
fn keyed_in(key: Key, id: &str, label: impl Into<Element>, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  let one_line = matches!(key, Key::OneLine);
  let key = Column::new()
    .id(format!("{id}-key"))
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
  let key = if one_line { key.overflow_visible() } else { key };
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

/// A value that is words; it wraps onto the next line when the row runs out of room.
pub(in crate::screens) fn words(id: &str, words: &str) -> Text {
  value(Text::new(words).id(id.to_owned()).variant(HubText::Value.role()))
}

/// A value that is a model's name or another identifier; one without a place to break is broken where it ends.
pub(in crate::screens) fn mono(id: &str, words: &str) -> Text {
  value(Text::new(words).id(id.to_owned()).variant(HubText::PanelMono.role()))
}

/// A value text takes the row's room and no more, so that what does not fit wraps instead of being cut.
fn value(text: Text) -> Text {
  text.flex(1.0).min_width(Dimension::Px(0.0))
}

/// A value that is names, each in a tag; they wrap onto the next line when the row runs out of room.
pub(in crate::screens) fn tags(id: &str, names: impl IntoIterator<Item = String>) -> Row {
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
pub(in crate::screens) fn on_this_computer(ctx: &Ctx, rows: Vec<Row>, after: Option<Element>) -> Column {
  let note = Text::new(&ctx.t("team_hub.profile.onComputerNote"))
    .id("team-hub-on-computer-note")
    .variant(HubText::PanelHint.role())
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
