//! Skill list header CrBUa and skill row Tv2Ei.
use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::link::Open,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};
const FILES: f32 = 64.0;
const VERSION: f32 = 190.0;
const SCOPE: f32 = 190.0;
const PUBLISHED: f32 = 100.0;
const CHEVRON: f32 = 14.0;
pub const ROW_HEIGHT: f32 = 62.0;
const HEADER_HEIGHT: f32 = 32.0;
fn line(id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::skills::cell_spacing())
    .padding(Padding::horizontal(crate::theme::skills::cell_spacing()))
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}
pub fn header(id: &str, labels: &[Arc<str>; 5]) -> Row {
  let widths = [None, Some(FILES), Some(VERSION), Some(SCOPE), Some(PUBLISHED)];
  line(id)
    .height(Dimension::Px(HEADER_HEIGHT))
    .with_children(labels.iter().zip(widths).map(|(label, width)| {
      let cell = Row::new().child(Text::new(label).variant(crate::theme::skills::heading()).nowrap());
      match width {
        None => cell.flex(1.0),
        Some(w) => cell.width(Dimension::Px(w)),
      }
    }))
    .child(Row::new().width(Dimension::Px(CHEVRON)))
}
pub struct SkillRow {
  id: Arc<str>,
  name: Arc<str>,
  description: Arc<str>,
  values: [Arc<str>; 4],
  version: Icon,
  scope: Icon,
  open: Open,
}
impl SkillRow {
  pub fn new(
    id: impl Into<Arc<str>>,
    name: impl Into<Arc<str>>,
    description: impl Into<Arc<str>>,
    values: [Arc<str>; 4],
    open: Open,
  ) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      description: description.into(),
      values,
      version: Icon::CircleDashed,
      scope: Icon::FolderGit2,
      open,
    }
  }
  pub fn marks(mut self, version: Icon, scope: Icon) -> Self {
    self.version = version;
    self.scope = scope;
    self
  }
}
impl From<SkillRow> for Element {
  fn from(row: SkillRow) -> Self {
    let cells = [
      (FILES, None),
      (VERSION, Some(row.version)),
      (SCOPE, Some(row.scope)),
      (PUBLISHED, None),
    ];
    let content = line(&row.id)
      .min_height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::symmetric(
        crate::theme::skills::cell_spacing(),
        ExtraSpacing::Loose.role(),
      ))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Fine.role())
          .child(Text::new(&row.name).variant(crate::theme::skills::name()).nowrap())
          .child(
            Text::new(&row.description)
              .variant(ExtraTypography::Note.role())
              .color(PaletteColor::TextMuted),
          ),
      )
      .with_children(cells.into_iter().zip(row.values).map(|((width, mark), label)| {
        Row::new()
          .width(Dimension::Px(width))
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .with_children(mark.map(|glyph| icon(glyph, IconSize::Base, PaletteColor::TextMuted)))
          .child(Text::new(&label).variant(TypographyStyle::Description))
      }))
      .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
    opening(row.open).id(row.id).child(content).into()
  }
}
