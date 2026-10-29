//! Settings' groups and rows (`App settings · redesign` `iUmo3` in the new `orchester.pen`, as `Appearance` `g6P4CU`
//! in `01 · General` draws them). A group is its heading in 12/600 `text_secondary` with an optional count or meta
//! after it and an action or a mark at its end, 8 over a box of rows: a 1 `border` stroke, radius 10, a 1 `border`
//! line between rows. A row is 11 over and under and 14 at its sides, 12 between its parts: an optional tile, the
//! label in 13/500 with at most one short hint 2 under it in 12 `text_muted` (or its tone's color), the rest of what
//! the setting does in an (i) tooltip after the label, and the control at the end. What a row says under itself (a
//! problem, a question, a code to copy) runs under it inside the box, 14 at its sides and 12 under it. A row that
//! opens a page is pressed as a whole and shows the `hover` fill under the pointer.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::opening,
  tooltip::Tooltip,
};
use crate::{
  navigation::link::Open,
  shell::status_bar::dot,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, SettingsSpacing, SettingsText},
};

/// A row's tile (`Tile` `BpusX` in `03 · Accounts`) and a page head's (`Tile` `KG1Td` in `03b · Account`).
const ROW_TILE: f32 = 28.0;
const HEAD_TILE: f32 = 36.0;

/// The color a row's hint is in: `text_muted`, or its tone's when it says how the setting stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HintTone {
  #[default]
  Muted,
  Warning,
  Danger,
  Success,
}

impl HintTone {
  const fn color(self) -> PaletteColor {
    match self {
      Self::Muted => PaletteColor::TextMuted,
      Self::Warning => PaletteColor::Warning,
      Self::Danger => PaletteColor::Danger,
      Self::Success => PaletteColor::Success,
    }
  }
}

/// A group: its heading and its rows in their box.
pub struct SettingGroup {
  id: Arc<str>,
  title: Arc<str>,
  meta: Option<Arc<str>>,
  end: Option<Element>,
  notice: Option<Element>,
  rows: Vec<Element>,
  note: Option<Arc<str>>,
}

impl SettingGroup {
  /// The group `id` (its heading `<id>-heading`, its title `<id>-title`, its box `<id>-rows`).
  pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      meta: None,
      end: None,
      notice: None,
      rows: Vec::new(),
      note: None,
    }
  }

  /// A notice between the heading and the box, 4 further from the box (`Not found` `PzLrb` in `09b · … not found`
  /// `NZECi`).
  pub fn notice(mut self, notice: impl Into<Element>) -> Self {
    self.notice = Some(notice.into());
    self
  }

  /// A count or when it was read, after the title in 12 `text_muted` (`Count` `e8kTSZ`, `Meta` `F5yRK`).
  pub fn meta(mut self, meta: impl Into<Arc<str>>) -> Self {
    self.meta = Some(meta.into());
    self
  }

  /// An action or a mark at the heading's end (`Add account` `Dt4uC`, `Person` `QOPs1`).
  pub fn end(mut self, end: impl Into<Element>) -> Self {
    self.end = Some(end.into());
    self
  }

  pub fn row(mut self, row: impl Into<Element>) -> Self {
    self.rows.push(row.into());
    self
  }

  pub fn rows(mut self, rows: impl IntoIterator<Item = Element>) -> Self {
    self.rows.extend(rows);
    self
  }

  /// What the whole group means, under its box in 12 `text_muted` (`Note` `XLcsr`).
  pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
    self.note = Some(note.into());
    self
  }
}

impl From<SettingGroup> for Element {
  fn from(group: SettingGroup) -> Self {
    let title = Text::new(&group.title)
      .id(format!("{}-title", group.id))
      .variant(SettingsText::GroupTitle.role())
      .nowrap();
    let meta = group.meta.map(|meta| {
      Text::new(&meta)
        .id(format!("{}-meta", group.id))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
    });
    let title = if meta.is_some() { title } else { title.flex(1.0) };
    let heading = Row::new()
      .id(format!("{}-heading", group.id))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(title)
      .with_children(meta)
      .with_children(group.end);
    // A group with no rows (plan limits nothing reported yet) is its heading and its note alone.
    let rows = (!group.rows.is_empty()).then(|| {
      let cells = group.rows.into_iter().enumerate().map(|(index, row)| {
        let cell = Column::new().width(Dimension::Pct(100.0)).child(row);
        if index == 0 {
          cell
        } else {
          cell.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        }
      });
      Column::new()
        .id(format!("{}-rows", group.id))
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Lg)
        .clip()
        .with_children(cells)
    });
    let notice = group.notice.map(|notice| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().bottom(SpacingSize::Xs))
        .child(notice)
    });
    Column::new()
      .id(group.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(heading)
      .with_children(notice)
      .with_children(rows)
      .with_children(group.note.map(|note| group_note(&format!("{}-note", group.id), &note)))
      .into()
  }
}

/// A note under a group or a page, in 12 `text_muted` (`Note` `SNxSa`).
pub fn group_note(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
}

/// A row of a group.
pub struct SettingRow {
  id: Arc<str>,
  label: Arc<str>,
  detail: Option<Element>,
  hint: Option<(Arc<str>, HintTone)>,
  info: Option<Arc<str>>,
  lead: Option<Element>,
  control: Option<Element>,
  below: Vec<Element>,
  dependent: bool,
  faded: bool,
  selected: bool,
  opens: Option<Open>,
}

impl SettingRow {
  /// The row of `id` (its frame `<id>-row`, its label `<id>-label`, its hint `<id>-hint`, its tooltip's anchor
  /// `<id>-info`).
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      detail: None,
      hint: None,
      info: None,
      lead: None,
      control: None,
      below: Vec::new(),
      dependent: false,
      faded: false,
      selected: false,
      opens: None,
    }
  }

  /// A value on a line of its own between the label and the hint, such as a program's path (`Path` `M2Pwq` in
  /// `09 · Settings · Browser` `j3Lre`).
  pub fn detail(mut self, detail: impl Into<Element>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  /// One short line under the label.
  pub fn hint(self, hint: impl Into<Arc<str>>) -> Self {
    self.toned_hint(hint, HintTone::Muted)
  }

  /// The hint in a tone that says how the setting stands.
  pub fn toned_hint(mut self, hint: impl Into<Arc<str>>, tone: HintTone) -> Self {
    self.hint = Some((hint.into(), tone));
    self
  }

  /// The rest of what the setting does, in the tooltip of an (i) after the label.
  pub fn info(mut self, info: impl Into<Arc<str>>) -> Self {
    self.info = Some(info.into());
    self
  }

  /// A tile or a mark before the label.
  pub fn lead(mut self, lead: impl Into<Element>) -> Self {
    self.lead = Some(lead.into());
    self
  }

  /// The control or the value at the row's end.
  pub fn control(mut self, control: impl Into<Element>) -> Self {
    self.control = Some(control.into());
    self
  }

  /// What the row says under itself, after what was added before.
  pub fn below(mut self, part: impl Into<Element>) -> Self {
    self.below.push(part.into());
    self
  }

  pub fn below_all(mut self, parts: impl IntoIterator<Item = Element>) -> Self {
    self.below.extend(parts);
    self
  }

  /// A setting that needs the one above it: it starts further in (`JMLwI`).
  pub fn dependent(mut self) -> Self {
    self.dependent = true;
    self
  }

  /// The label in `text_muted`: what it names is not in use now (`Address` `GXDxS`, the server off).
  pub fn faded(mut self) -> Self {
    self.faded = true;
    self
  }

  /// The row of the page shown, in the `hover` fill.
  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  /// A press on the row opens a page with `open`.
  pub fn opens(mut self, open: Open) -> Self {
    self.opens = Some(open);
    self
  }

  /// The label with its (i), its detail and the hint under them.
  fn text(&mut self, ctx: &mut Ctx) -> Column {
    let label = Text::new(&self.label)
      .id(format!("{}-label", self.id))
      .variant(ExtraTypography::Strong.role())
      .color(if self.faded {
        PaletteColor::TextMuted
      } else {
        PaletteColor::TextPrimary
      });
    let info = self.info.clone().map(|info| {
      let mark = Row::new()
        .id(format!("{}-info", self.id))
        .describe("a11y_name", info.clone())
        .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted));
      Tooltip::new(format!("{}-info", self.id), info).wrap(ctx, mark)
    });
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(label)
          .with_children(info),
      )
      .with_children(self.detail.take())
      .with_children(self.hint.as_ref().map(|(hint, tone)| {
        Text::new(hint)
          .id(format!("{}-hint", self.id))
          .variant(TypographyStyle::Description)
          .color(tone.color())
          .width(Dimension::Pct(100.0))
      }))
  }

  pub fn render(mut self, ctx: &mut Ctx) -> Element {
    let text = self.text(ctx);
    let padding = Padding::symmetric(ExtraSpacing::Inset.role(), SettingsSpacing::RowY.role());
    let padding = if self.dependent {
      padding.left(SettingsSpacing::Dependent.role())
    } else {
      padding
    };
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .padding(padding)
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .with_children(self.lead)
      .child(text)
      .with_children(self.control);
    let line: Element = match self.opens {
      Some(open) => {
        let row = opening(open)
          .id(format!("{}-open", self.id))
          .width(Dimension::Pct(100.0))
          .hovered_style(Style::new().background(ExtraColor::Hover.role()))
          .child(line);
        if self.selected {
          row.background(ExtraColor::Hover.role()).into()
        } else {
          row.into()
        }
      }
      None => line.into(),
    };
    let below = (!self.below.is_empty()).then(|| {
      Column::new()
        .id(format!("{}-below", self.id))
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(ExtraSpacing::Inset.role())
            .right(ExtraSpacing::Inset.role())
            .bottom(SpacingSize::Md),
        )
        .spacing(SpacingSize::Sm)
        .with_children(self.below)
    });
    Column::new()
      .id(format!("{}-row", self.id))
      .width(Dimension::Pct(100.0))
      .describe("selected", if self.selected { "true" } else { "false" })
      .child(line)
      .with_children(below)
      .into()
  }
}

/// A read-only value at a row's end, in 12.5 `text_secondary` (`Value` `zVmOU`).
pub fn value(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Lead.role())
    .nowrap()
}

/// A path, an address or a token at a row's end, in Geist Mono 12 `text_secondary`, cut with an ellipsis when it is
/// too long (`Text` `M0FfD`).
pub fn mono_value(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::PropertyMono.role())
    .color(PaletteColor::TextSecondary)
    .nowrap()
    .min_width(Dimension::Px(0.0))
    .text_overflow(TextOverflow::Elipsis)
}

/// A state as a 6 dot and its words in 12 (`Status` `i0iw5`): the words in `color`, `text_secondary` unless the state
/// asks for attention.
pub fn status(id: &str, (dot_color, color): (PaletteColor, PaletteColor), words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(dot(dot_color))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .color(color)
        .nowrap(),
    )
}

/// A row's chevron: the row opens a page (`Open` `w8dpO`).
pub fn opens_mark() -> Text {
  icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)
}

/// A 28 tile with `content` in its middle (`Tile` `BpusX`).
pub fn row_tile(content: impl Into<Element>) -> Row {
  tile(ROW_TILE, RadiusSize::Md, content)
}

/// A page head's 36 tile (`Tile` `KG1Td`).
pub fn head_tile(content: impl Into<Element>) -> Row {
  tile(HEAD_TILE, ExtraRadius::Tile.role(), content)
}

fn tile(side: f32, radius: RadiusSize, content: impl Into<Element>) -> Row {
  Row::new()
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(radius)
    .child(content)
}

/// A mark with words in 11.5 `text_muted` after it, 5 apart: who alone may change a group (`Person` `QOPs1`), a value
/// that is fixed (`Fixed` `qLjNX`), a feature not in this app yet (`Unavailable` `H0lYd`).
pub fn marked_words(id: &str, glyph: Icon, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Xs, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
}
