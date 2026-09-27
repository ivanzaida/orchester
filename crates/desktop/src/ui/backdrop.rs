//! Dialog backdrop (`a8U756`): the scrim over the screen with the dialog centred in it, as the
//! design draws every backdrop over its 1440 x 960 screen frame and nothing else. The window
//! chrome around the screen stays outside the modal layer (design doc "Window chrome"): the
//! title bar above and the status bar below are neither dimmed nor covered, so the status bar
//! keeps its hover and clicks while a dialog is open. It never takes the focus, so Tab stays in
//! the dialog. A dialog never touches the left and right window edges: a dialog that can shrink
//! keeps `ExtraSpacing::DialogMargin` from them in a narrow window.

use lurq::{
  components::Column,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, STATUS_BAR_HEIGHT, TITLE_BAR_HEIGHT};

/// `dialog` over the scrim; mounted in a `Modal` that targets the whole window.
pub fn backdrop(dialog: impl Into<Element>) -> Element {
  let scrim = Column::new()
    .id("dialog-backdrop")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_horizontal(ExtraSpacing::DialogMargin.role())
    .background(ExtraColor::Scrim.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(dialog);
  // The bands above and below the scrim belong to the chrome: the pointer passes through them.
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(Dimension::Px(TITLE_BAR_HEIGHT))
    .padding_bottom(Dimension::Px(STATUS_BAR_HEIGHT))
    .hit_test(HitTestBehavior::ContentOnly)
    .child(scrim)
    .into()
}
