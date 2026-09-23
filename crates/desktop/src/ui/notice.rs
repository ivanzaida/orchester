//! Inline notice (`q5LzI`): an icon beside a short heading and its detail, optionally with
//! trailing actions. The emphasized form is the runner connection notice (`Dobsh`, `ctSIe`,
//! `PVhFs`): the icon in the text color, a medium heading and actions on the right.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

pub struct Notice {
  icon: Icon,
  heading: Arc<str>,
  detail: Arc<str>,
  emphasized: bool,
  actions: Vec<Element>,
  id: Option<&'static str>,
}

impl Notice {
  pub fn new(icon: Icon, heading: Arc<str>, detail: Arc<str>) -> Self {
    Self {
      icon,
      heading,
      detail,
      emphasized: false,
      actions: Vec::new(),
      id: None,
    }
  }

  pub fn emphasized(mut self) -> Self {
    self.emphasized = true;
    self
  }

  /// Content on the right of the copy: buttons, a countdown, a dismiss control.
  pub fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }

  /// Element id for tooling (`lurq_read_tree`, `lurq_interact`).
  pub fn id(mut self, id: &'static str) -> Self {
    self.id = Some(id);
    self
  }
}

impl From<Notice> for Element {
  fn from(notice: Notice) -> Self {
    let (icon_color, heading) = if notice.emphasized {
      (PaletteColor::TextPrimary, ExtraTypography::NoticeHeading.role())
    } else {
      (PaletteColor::TextSecondary, TypographyStyle::FieldLabel)
    };
    let copy = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(&notice.heading)
          .variant(heading)
          .color(PaletteColor::TextPrimary),
      )
      .child(Text::new(&notice.detail).variant(ExtraTypography::Detail.role()));
    let mut content = Row::new()
      .flex(1.0)
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(copy);
    if !notice.actions.is_empty() {
      content = content.child(
        Row::new()
          .spacing(ExtraSpacing::Stack.role())
          .align_items(Alignment::Center)
          .with_children(notice.actions),
      );
    }
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Notice.role())
      .child(icon(notice.icon, IconSize::Notice, icon_color))
      .child(content);
    if let Some(id) = notice.id {
      row = row.id(id);
    }
    row.into()
  }
}

pub fn notice(glyph: Icon, heading: &str, detail: &str) -> Element {
  Notice::new(glyph, heading.into(), detail.into()).into()
}
