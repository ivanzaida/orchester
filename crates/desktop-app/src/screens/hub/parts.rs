//! The hub's shared parts: a group's head over its rows (`Head` `dUbs7`), rows in a box parted by lines (`Rows`
//! `QpdBr`), a list row with its tile, label, hint and what it shows at its end (`Builder` `fYNhS`: 10 over and
//! under, 14 at its sides, 12 apart, the `hover` fill under the pointer), a form row with its label and hint at the
//! start and its control at the end (`Profile name` `BzwDA`), a quiet note (`Note` `HVD56`) and a status with its dot
//! (`Status` `B6JYf`).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressed},
  },
};

/// A list row's tile (`Tile` `uEJC1`), and a page head's (`NWKKM`).
const TILE: f32 = 28.0;
const PAGE_TILE: f32 = 36.0;
/// A status dot (`Dot` `k934A`).
const DOT: f32 = 6.0;

/// The tone of a status's dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
  Success,
  Warning,
  Danger,
  Muted,
}

impl Tone {
  fn color(self) -> PaletteColor {
    match self {
      Self::Success => PaletteColor::Success,
      Self::Warning => PaletteColor::Warning,
      Self::Danger => PaletteColor::Danger,
      Self::Muted => PaletteColor::TextMuted,
    }
  }
}

/// A group's or a section's head: its title, then what it counts in quiet words, then `end` at its right.
pub(crate) fn head(id: &str, title: &str, count: Option<&str>, end: Option<Element>) -> Row {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .with_children(count.map(|count| {
      Text::new(count)
        .id(format!("{id}-count"))
        .variant(HubText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
    }))
    .with_children(count.is_none().then(|| Row::new().flex(1.0)))
    .with_children(end)
}

/// A group: its head over `content`, 8 apart.
pub(crate) fn group(id: &str, head: Row, content: impl Into<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(head)
    .child(content)
}

/// `rows` in a box with rounded corners, a line between each and the next.
pub(crate) fn boxed(id: &str, rows: impl IntoIterator<Item = Element>) -> Column {
  let rows = rows.into_iter().enumerate().map(|(index, row)| {
    let frame = Column::new().width(Dimension::Pct(100.0)).child(row);
    if index == 0 {
      frame
    } else {
      frame.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .with_children(rows)
}

/// A tile with `glyph`, 28 beside a list row's label or 36 in a page head.
pub(crate) fn tile(glyph: Icon, page: bool) -> Row {
  tile_in(glyph, page, PaletteColor::TextSecondary)
}

/// [`tile`] with its glyph in `color`, e.g. an import result's check in `success` (`zEejx` in 03c).
pub(crate) fn tile_in(glyph: Icon, page: bool, color: PaletteColor) -> Row {
  let side = if page { PAGE_TILE } else { TILE };
  Row::new()
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(icon(glyph, if page { IconSize::Xl } else { IconSize::Md }, color))
}

/// A sub-head inside a group's rows: a role's, a pipeline's or, indented with its glyph, a step's (`ksfLm`, `mwisw`
/// in `jtiqe`): 30 high on `surface_raised`, its label in 12/600 `text_secondary`, then `count` in quiet words.
pub(crate) fn sub_head(id: &str, label: &str, count: Option<&str>, step: bool) -> Row {
  let start = if step { HubSpacing::Indent } else { HubSpacing::Row };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::SubY.role())
        .bottom(HubSpacing::SubY.role())
        .left(start.role())
        .right(HubSpacing::Row.role()),
    )
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .with_children(step.then(|| icon(Icon::CornerDownRight, IconSize::Sm, PaletteColor::TextMuted)))
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .with_children(count.map(|count| Text::new(count).variant(HubText::Hint.role()).nowrap()))
}

/// A list row: a tile, its label and hint, then `end` and a chevron when it opens something.
pub(crate) struct Entry {
  id: Arc<str>,
  tile: Option<Icon>,
  label: Arc<str>,
  hint: Option<(Arc<str>, Option<PaletteColor>)>,
  end: Vec<Element>,
  press: Option<Press>,
  elsewhere: bool,
  indent: bool,
  chosen: bool,
  code: bool,
  chevron: bool,
}

impl Entry {
  pub(crate) fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      tile: None,
      label: label.into(),
      hint: None,
      end: Vec::new(),
      press: None,
      elsewhere: false,
      indent: false,
      chosen: false,
      code: false,
      chevron: true,
    }
  }

  /// The row is the one chosen, filled with `hover` (`Db46B` in 02b's sources, `pHCCx` in 02d).
  pub(crate) const fn chosen(mut self, chosen: bool) -> Self {
    self.chosen = chosen;
    self
  }

  /// The label is code, e.g. a variable's `{{task.id}}` (`wuoOU` in 02d).
  pub(crate) const fn code(mut self) -> Self {
    self.code = true;
    self
  }

  /// A press chooses the row in place rather than opening something: no chevron at its end.
  pub(crate) const fn in_place(mut self) -> Self {
    self.chevron = false;
    self
  }

  pub(crate) fn tile(mut self, glyph: Icon) -> Self {
    self.tile = Some(glyph);
    self
  }

  pub(crate) fn hint(mut self, hint: impl Into<Arc<str>>) -> Self {
    self.hint = Some((hint.into(), None));
    self
  }

  /// The hint in `color`, e.g. a missing connection's warning (06b).
  pub(crate) fn hint_in(mut self, hint: impl Into<Arc<str>>, color: PaletteColor) -> Self {
    self.hint = Some((hint.into(), Some(color)));
    self
  }

  pub(crate) fn end(mut self, part: impl Into<Element>) -> Self {
    self.end.push(part.into());
    self
  }

  /// What a press on the row does; the row then ends in a chevron.
  pub(crate) fn press(mut self, press: Press) -> Self {
    self.press = Some(press);
    self
  }

  /// A press opens another tab or page: the row ends in an arrow out instead of a chevron (`↗` in 01c's side).
  pub(crate) fn elsewhere(mut self) -> Self {
    self.elsewhere = true;
    self
  }

  /// The row sits under a step's sub-head, indented as far as it (`XWrFf` in `w31Kt`).
  pub(crate) fn indented(mut self) -> Self {
    self.indent = true;
    self
  }
}

impl From<Entry> for Element {
  fn from(entry: Entry) -> Self {
    let label = Text::new(&entry.label)
      .id(format!("{}-label", entry.id))
      .variant(if entry.code {
        HubText::Code.role()
      } else {
        ExtraTypography::Strong.role()
      })
      .nowrap();
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(label)
      .with_children(entry.hint.map(|(hint, color)| {
        let hint = Text::new(&hint)
          .id(format!("{}-hint", entry.id))
          .variant(HubText::Hint.role())
          .width(Dimension::Pct(100.0));
        match color {
          Some(color) => hint.color(color),
          None => hint,
        }
      }));
    let opens = (entry.chevron && entry.press.is_some()).then_some(if entry.elsewhere {
      Icon::ArrowUpRight
    } else {
      Icon::ChevronRight
    });
    let start = if entry.indent {
      HubSpacing::Indent
    } else {
      HubSpacing::Row
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(HubSpacing::RowY.role())
          .bottom(HubSpacing::RowY.role())
          .left(start.role())
          .right(HubSpacing::Row.role()),
      )
      .spacing(lurq::app::theme::SpacingSize::Md)
      .align_items(Alignment::Center)
      .with_children(entry.tile.map(|glyph| tile(glyph, false)))
      .child(text)
      .with_children(entry.end)
      .with_children(opens.map(|glyph| icon(glyph, IconSize::Md, PaletteColor::TextMuted)));
    let row = if entry.chosen {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    };
    match entry.press {
      Some(press) => pressed(press)
        .id(entry.id.clone())
        .width(Dimension::Pct(100.0))
        .describe("state", if entry.chosen { "chosen" } else { "enabled" })
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .child(row)
        .into(),
      None => row.id(entry.id.clone()).into(),
    }
  }
}

/// A form row: `label` and its `hint` at the start, `control` at the end; the row's element id is `<id>-row`.
pub(crate) fn field(id: &str, label: &str, hint: Option<&str>, control: impl Into<Element>) -> Row {
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Center)
    .child(field_text(id, label, hint))
    .child(control)
}

/// A form row whose control runs under its label and hint, with `meta` at its right (`Role instructions` `mR3hs`).
pub(crate) fn stacked_field(
  id: &str,
  (label, hint): (&str, Option<&str>),
  control: impl Into<Element>,
  meta: Option<&str>,
) -> Row {
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Start)
    .child(
      field_text(id, label, hint)
        .child(Row::new().height(Dimension::Px(ExtraSpacing::Snug.value())))
        .child(control),
    )
    .with_children(meta.map(|meta| Text::new(meta).variant(HubText::Hint.role()).nowrap()))
}

fn field_text(id: &str, label: &str, hint: Option<&str>) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(ExtraTypography::Strong.role()),
    )
    .with_children(hint.map(|hint| {
      Text::new(hint)
        .id(format!("{id}-hint"))
        .variant(HubText::Hint.role())
        .width(Dimension::Pct(100.0))
    }))
}

/// A quiet note: a 12 `glyph` and `text` in 12 `text_muted`, 8 apart, wrapping.
pub(crate) fn note(id: &str, glyph: Icon, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(HubText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A status: a dot in `tone` and its `text`, 6 apart.
pub(crate) fn status(id: &str, tone: Tone, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("tone", format!("{tone:?}").to_lowercase())
    .child(
      Row::new()
        .width(Dimension::Px(DOT))
        .height(Dimension::Px(DOT))
        .rounded(ExtraRadius::Full.role())
        .background(tone.color()),
    )
    .child(Text::new(text).variant(HubText::Status.role()).nowrap())
}

/// Quiet words at a row's end, e.g. *3 pipelines* (`UOYXm` in `fYNhS`).
pub(crate) fn quiet(text: &str) -> Text {
  Text::new(text).variant(HubText::Hint.role()).nowrap()
}
