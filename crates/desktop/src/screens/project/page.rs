//! Topbar and page headings shared by the project screens: the topbar and heading of Agents and
//! Connections (`Dy8Cy`, `Cj6Dz` in `a4Lqb`; `c0U8I`, `eEaSg` in `WPmtS`), and the screen
//! heading of Tasks and Pull requests (`LIfyO` in `J9467`, `fXrG1` in `wGtxT`).

use lurq::{
  app::theme::{BorderSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Topbar height (`Dy8Cy`) and the height of its buttons (`ggfFj`).
const TOPBAR_HEIGHT: f32 = 64.0;
pub const TOPBAR_BUTTON_HEIGHT: f32 = 36.0;

/// The breadcrumb on the left, `trailing` (status text, actions) on the right.
pub fn topbar(breadcrumb: &str, trailing: impl IntoIterator<Item = Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(breadcrumb).variant(TypographyStyle::Description).nowrap())
    .child(Spacer::new().flex(1.0))
    .with_children(trailing)
    .into()
}

/// Screen title and its description, as Tasks and Pull requests show it.
pub fn screen_heading(title: &str, description: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::HeadingTop.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(ExtraSpacing::HeadingBottom.role())
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(title).variant(ExtraTypography::ScreenTitle.role()))
    .child(Text::new(description).variant(TypographyStyle::Description))
    .into()
}

/// Page title and its description.
pub fn heading(title: &str, description: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(title).variant(ExtraTypography::PageTitle.role()))
    .child(Text::new(description).variant(TypographyStyle::Description))
    .into()
}
