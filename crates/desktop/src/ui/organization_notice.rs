//! Organization notices in rVJoA, Ncjvt, DtpHJ and tag impact frames.
use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Text},
  node::dimension::Dimension,
};

use crate::theme::ExtraTypography;

pub fn organization_notice(title: &str, detail: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(Text::new(title).variant(ExtraTypography::NoticeHeading.role()))
    .child(Text::new(detail).variant(ExtraTypography::Explanation.role()))
}
