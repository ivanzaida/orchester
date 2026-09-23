//! Static parts of 01.03 around the project form: topbar, page heading and the project
//! ownership explanation.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Topbar height (`MYfj5`).
const TOPBAR_HEIGHT: f32 = 64.0;

/// Topbar (`MYfj5`) with the breadcrumb. There is no setup draft, so no save status.
pub fn topbar(ctx: &Ctx, name: &str) -> Element {
  let breadcrumb = if name.is_empty() {
    ctx.t("setup.project.breadcrumb")
  } else {
    ctx.t_args("setup.project.breadcrumbNamed", [("name", name)])
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&breadcrumb).variant(TypographyStyle::Description))
    .into()
}

/// Page heading (`b0m9CN`).
pub fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("setup.project.title")).variant(ExtraTypography::PageTitle.role()))
    .child(Text::new(&ctx.t("setup.project.description")).variant(TypographyStyle::Description))
    .into()
}

/// Project ownership explanation (`j452o`).
pub fn ownership(ctx: &Ctx, name: &str) -> Element {
  let detail = if name.is_empty() {
    ctx.t("setup.project.ownershipDetail")
  } else {
    ctx.t_args("setup.project.ownershipDetailNamed", [("name", name)])
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("setup.project.ownershipTitle")).variant(ExtraTypography::Subheading.role()))
    .child(
      Text::new(&detail)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}
