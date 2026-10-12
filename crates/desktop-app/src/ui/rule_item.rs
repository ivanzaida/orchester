//! Rule item zmP3o: name, version and timing in one selected or openable widget.
use std::sync::Arc;

use lurq::{
  app::theme::{RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Text},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::pressable::pressable;
use crate::theme::{ExtraColor, ExtraSpacing, instructions};

pub struct RuleItem {
  id: Arc<str>,
  name: Arc<str>,
  version: Arc<str>,
  meta: Arc<str>,
  selected: bool,
  open: Arc<dyn Fn() + Send + Sync>,
}
impl RuleItem {
  pub fn new(
    id: impl Into<Arc<str>>,
    name: impl Into<Arc<str>>,
    version: impl Into<Arc<str>>,
    meta: impl Into<Arc<str>>,
    selected: bool,
    open: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      version: version.into(),
      meta: meta.into(),
      selected,
      open: Arc::new(open),
    }
  }
}
impl From<RuleItem> for Element {
  fn from(item: RuleItem) -> Element {
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .spacing(ExtraSpacing::Fine.role())
      .child(Text::new(&item.name).variant(instructions::rule_name()))
      .child(Text::new(&item.version).variant(TypographyStyle::Caption))
      .child(Text::new(&item.meta).variant(TypographyStyle::Caption));
    if item.selected {
      return body
        .id(item.id)
        .background(ExtraColor::Hover.role())
        .rounded(RadiusSize::Md)
        .into();
    }
    pressable(move || (item.open)())
      .id(item.id)
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(body)
      .into()
  }
}
