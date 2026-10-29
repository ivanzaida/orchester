//! The shapes skeletons share: a line of words in bones, a count, a state, a switch, and a row group's heading,
//! outline and rows in the geometry of `ui::row_group` (`Published` `pnnmh` in `x1lmXb`), so the real rows replace
//! them in place (`D · Bones` in `13 · Patterns` `AOQZ3`: count 14×8, 13 px words 10 high, 12 px 9, 11 px 8).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{bone, round_bone};
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, GroupText, IconSize},
  ui::icon::{Icon, icon},
};

/// A count's bone (`count 14×8`).
const COUNT: (f32, f32) = (14.0, 8.0);
/// A state's dot (`Status` `i0iw5`).
const DOT: f32 = 6.0;
/// A switch's track (`K8ItM`).
const SWITCH: (f32, f32) = (28.0, 16.0);

/// A line of words in `style`, `width` long: as high as the line, its bone in the middle as high as the design draws
/// words of that size.
pub fn line(ctx: &Ctx, style: impl Into<TypographyStyle>, width: f32) -> Row {
  let text = ctx.theme().typography().get(style);
  let high = match text.font_size {
    size if size >= 20.0 => 16.0,
    size if size >= 15.0 => 12.0,
    size if size >= 13.0 => 10.0,
    size if size >= 12.0 => 9.0,
    _ => 8.0,
  };
  Row::new()
    .height(Dimension::Px(text.font_size * text.line_height))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .child(bone(width, high))
}

/// A count beside a title or a tab.
pub fn count() -> Column {
  bone(COUNT.0, COUNT.1)
}

/// A state at a row's end: its dot and its words, `width` long, in 12.
pub fn status(ctx: &Ctx, width: f32) -> Row {
  Row::new()
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(round_bone(DOT))
    .child(line(ctx, TypographyStyle::Description, width))
}

/// A switch whose state is not known yet: a bone the size of its track.
pub fn switch() -> Column {
  bone(SWITCH.0, SWITCH.1)
}

/// A field whose value is not read yet: its frame `width` wide, as high as a control, a bone of `value` in it.
pub fn field(ctx: &Ctx, width: f32, value: f32) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .background(PaletteColor::SurfaceInput)
    .child(line(ctx, TypographyStyle::Description, value))
}

/// [`field`] with its unit real at its end (`tokens`, `%`).
pub fn unit_field(ctx: &Ctx, width: f32, value: f32, unit: &str) -> Row {
  field(ctx, width, value).child(Spacer::new().flex(1.0)).child(
    Text::new(unit)
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
}

/// [`field`] that opens a list: its chevron real at its end.
pub fn select(ctx: &Ctx, width: f32, value: f32) -> Row {
  field(ctx, width, value).child(Spacer::new().flex(1.0)).child(icon(
    Icon::ChevronDown,
    IconSize::Md,
    PaletteColor::TextMuted,
  ))
}
/// A button that comes with the data: a bone of its size.
pub fn button(width: f32) -> Column {
  bone(width, ControlHeight::Md.value()).rounded(RadiusSize::Md)
}

/// A row's chevron, real: the row will open a page.
pub fn chevron() -> Text {
  icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)
}

/// Words that stay real, or the bone of words not read yet, so long.
#[derive(Clone, Debug, PartialEq)]
pub enum Words {
  Real(Arc<str>),
  Bone(f32),
}

impl Words {
  fn line(self, ctx: &Ctx, style: TypographyStyle) -> Element {
    match self {
      Self::Real(words) => Text::new(&words).variant(style).nowrap().into(),
      Self::Bone(width) => line(ctx, style, width).into(),
    }
  }
}

/// A row group's outline around `rows`, a line between them (`ui::row_group`).
pub fn outline(id: &str, rows: Vec<Element>) -> Column {
  let lines = rows.into_iter().enumerate().map(|(index, row)| {
    let line = Column::new().width(Dimension::Pct(100.0)).child(row);
    if index == 0 {
      line
    } else {
      line.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  Column::new()
    .id(format!("{id}-rows"))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .with_children(lines)
}

/// Rows that open a page, in their outline: each a label and a hint of the widths `rows` gives, and a chevron; the
/// lower ones fade.
pub fn list(ctx: &Ctx, id: &str, rows: &[(f32, f32)]) -> Column {
  let rows = rows
    .iter()
    .enumerate()
    .map(|(index, (label, hint))| {
      BoneRow::new(format!("{id}-{index}"), Words::Bone(*label))
        .hint(Words::Bone(*hint))
        .end(chevron())
        .build(ctx)
    })
    .collect();
  outline(id, super::fading(rows).into_iter().map(Element::from).collect())
}

/// Lines of words in 13, as long as `widths` says in turn, 8 apart, the lower ones fading: what a panel says once it
/// is read, where no frame draws its rows.
pub fn lines(ctx: &Ctx, widths: &[f32]) -> Column {
  let lines = widths
    .iter()
    .map(|width| line(ctx, TypographyStyle::Body, *width))
    .collect();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(super::fading(lines))
}

/// A row group: its title, real or a bone, a count's bone when it is `counted`, what acts on it at its end, and
/// `rows` in their outline.
pub fn group(ctx: &Ctx, id: &str, (title, counted): (Words, bool), end: Option<Element>, rows: Vec<Element>) -> Column {
  let heading = Row::new()
    .id(format!("{id}-heading"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(title.line(ctx, GroupText::Title.role()))
    .with_children(counted.then(count))
    .child(Spacer::new().flex(1.0))
    .with_children(end);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(heading)
    .child(outline(id, rows))
}

/// How a row stands in its group, as `ui::row_group::GroupRow` spaces it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Shape {
  /// A label over a hint, a control or a chevron at its end.
  #[default]
  Plain,
  /// Marks around its label, or only words at its end.
  Marked,
  /// A tile at its start.
  Tiled,
}

/// A row of a row group in bones: its mark, its label and hint, and what is at its end, each real or a bone.
pub struct BoneRow {
  id: Arc<str>,
  lead: Option<Element>,
  label: Words,
  hint: Option<Words>,
  end: Vec<Element>,
  shape: Shape,
}

impl BoneRow {
  pub fn new(id: impl Into<Arc<str>>, label: Words) -> Self {
    Self {
      id: id.into(),
      lead: None,
      label,
      hint: None,
      end: Vec::new(),
      shape: Shape::Plain,
    }
  }

  /// A mark at its start; marks around its label: 12 between its parts.
  pub fn marked(mut self, lead: impl Into<Element>) -> Self {
    self.lead = Some(lead.into());
    self.shape = Shape::Marked;
    self
  }

  /// A tile at its start, 11 above and below it (`Starter · Feature delivery` `Asx53`).
  pub fn tiled(mut self, tile: impl Into<Element>) -> Self {
    self.lead = Some(tile.into());
    self.shape = Shape::Tiled;
    self
  }

  /// The line under the label.
  pub fn hint(mut self, hint: Words) -> Self {
    self.hint = Some(hint);
    self
  }

  /// What is at its end, after what was added before.
  pub fn end(mut self, end: impl Into<Element>) -> Self {
    self.end.push(end.into());
    self
  }

  pub fn build(self, ctx: &Ctx) -> Row {
    let vertical = match self.shape {
      Shape::Tiled => ExtraSpacing::TiledRow.role(),
      Shape::Marked if self.hint.is_none() => ExtraSpacing::Loose.role(),
      Shape::Plain | Shape::Marked => SpacingSize::Sm,
    };
    let between = match self.shape {
      Shape::Plain => SpacingSize::Lg,
      Shape::Marked | Shape::Tiled => SpacingSize::Md,
    };
    let middle = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(self.label.line(ctx, ExtraTypography::Strong.role()))
      .with_children(self.hint.map(|hint| hint.line(ctx, GroupText::Hint.role())));
    Row::new()
      .id(self.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), vertical))
      .spacing(between)
      .align_items(Alignment::Center)
      .with_children(self.lead)
      .child(middle)
      .with_children(self.end)
  }
}
