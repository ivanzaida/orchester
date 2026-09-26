//! Dialog backdrop (`a8U756`): the scrim over the whole window, with the dialog centred in the
//! screen below the custom title bar, as the design places dialogs within the 1440 x 960 screen.
//! A dialog never touches the left and right window edges: a dialog that can shrink keeps
//! `ExtraSpacing::DialogMargin` from them in a narrow window.

use lurq::{
  components::Column,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, TITLE_BAR_HEIGHT};

pub fn backdrop(dialog: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(Dimension::Px(TITLE_BAR_HEIGHT))
    .padding_horizontal(ExtraSpacing::DialogMargin.role())
    .background(ExtraColor::Scrim.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(dialog)
    .into()
}
