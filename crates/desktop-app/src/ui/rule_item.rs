//! Rule item zmP3o: name, version and timing in one selected or openable widget.
use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::opening,
};
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraSpacing, IconSize, instructions},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuleState {
  Draft,
  FirstDraft,
  Published,
}
pub struct RuleItem {
  id: Arc<str>,
  name: Arc<str>,
  version: Arc<str>,
  meta: Arc<str>,
  selected: bool,
  state: RuleState,
  open: Open,
}
impl RuleItem {
  pub fn new(
    id: impl Into<Arc<str>>,
    name: impl Into<Arc<str>>,
    version: impl Into<Arc<str>>,
    meta: impl Into<Arc<str>>,
    selected: bool,
    open: Open,
  ) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      version: version.into(),
      meta: meta.into(),
      selected,
      state: RuleState::Draft,
      open,
    }
  }
  /// A published version has the green check of zmP3o; drafts use the editing glyph.
  pub fn published(mut self) -> Self {
    self.state = RuleState::Published;
    self
  }
  /// A first draft has the unpublished dashed mark.
  pub fn first_draft(mut self) -> Self {
    self.state = RuleState::FirstDraft;
    self
  }
}
impl From<RuleItem> for Element {
  fn from(item: RuleItem) -> Element {
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .spacing(ExtraSpacing::Fine.role())
      .child(Text::new(&item.name).variant(instructions::rule_name()))
      .child(
        Row::new()
          .spacing(ExtraSpacing::Fine.role())
          .align_items(lurq::layout::Alignment::Center)
          .child(icon(
            if item.state == RuleState::Published {
              Icon::CircleCheck
            } else if item.state == RuleState::FirstDraft {
              Icon::CircleDashed
            } else {
              Icon::FilePen
            },
            IconSize::Xs,
            if item.state == RuleState::Published {
              PaletteColor::Success
            } else {
              PaletteColor::TextMuted
            },
          ))
          .child(Text::new(&item.version).variant(TypographyStyle::Caption)),
      )
      .child(Text::new(&item.meta).variant(TypographyStyle::Caption));
    if item.selected {
      return body
        .id(item.id)
        .background(ExtraColor::Hover.role())
        .rounded(RadiusSize::Md)
        .into();
    }
    opening(item.open)
      .id(item.id)
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(body)
      .into()
  }
}
