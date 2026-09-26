//! Recovery panel (`iN90G`): icon, title, explanation and an optional action, on a bordered
//! surface. Used for empty and failure states.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Panel width (`iN90G`).
const PANEL_WIDTH: f32 = 416.0;

pub struct RecoveryPanel {
  icon: Icon,
  title: String,
  explanation: String,
  details: Vec<Element>,
  action: Option<Element>,
  width: f32,
}

impl RecoveryPanel {
  pub fn new(icon: Icon, title: &str, explanation: &str) -> Self {
    Self {
      icon,
      title: title.to_owned(),
      explanation: explanation.to_owned(),
      details: Vec::new(),
      action: None,
      width: PANEL_WIDTH,
    }
  }

  /// Extra content under the explanation, such as the path involved.
  pub fn detail(mut self, detail: impl Into<Element>) -> Self {
    self.details.push(detail.into());
    self
  }

  /// A panel wider than the component's 416, as a screen places it (e.g. 520 in 08.D `MBmo4`).
  pub fn width(mut self, width: f32) -> Self {
    self.width = width;
    self
  }

  pub fn action(mut self, action: Element) -> Self {
    self.action = Some(action);
    self
  }
}

impl From<RecoveryPanel> for Element {
  fn from(panel: RecoveryPanel) -> Self {
    let mut column = Column::new()
      .width(Dimension::Px(panel.width))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Group.role())
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(icon(panel.icon, IconSize::Panel, PaletteColor::TextSecondary))
      .child(Text::new(&panel.title).variant(ExtraTypography::Subheading.role()))
      // 12 muted at 1.6, as the dialog explanation (`VSkhh`).
      .child(Text::new(&panel.explanation).variant(ExtraTypography::Explanation.role()))
      .with_children(panel.details);
    if let Some(action) = panel.action {
      column = column.child(Column::new().padding_top(SpacingSize::Sm).child(action));
    }
    column.into()
  }
}
