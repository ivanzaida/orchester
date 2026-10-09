//! Meter (`Meter` `wtAQI`): how much of something is used, as a 4 high `border` track with the used part
//! filled from its start, both radius 2. The fill is `text_secondary`; a limit that was reached fills it in
//! `warning` (`Meter · tokens` in `Raise the limit` `xHs82`). It is as wide as its parent.

use lurq::{
  app::theme::PaletteColor,
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};

use crate::theme::ExtraRadius;

/// The track's height (`wtAQI`).
const HEIGHT: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterTone {
  /// `text_secondary`: a share of something.
  Plain,
  /// `warning`: a limit that was reached.
  Reached,
}

/// A meter of `used` out of `of`, with the element id `id`; more than `of` fills the whole track.
pub fn meter(id: &str, (used, of): (u64, u64), tone: MeterTone) -> Element {
  // A share of a few hundred pixels: f32 is precise enough.
  let percent = if of == 0 {
    100.0
  } else {
    (used as f32 / of as f32 * 100.0).min(100.0)
  };
  let fill = match tone {
    MeterTone::Plain => PaletteColor::TextSecondary,
    MeterTone::Reached => PaletteColor::Warning,
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .background(PaletteColor::Border)
    .rounded(ExtraRadius::Meter.role())
    .describe("share", format!("{percent:.0}"))
    .child(
      Column::new()
        .id(format!("{id}-fill"))
        .width(Dimension::Pct(percent))
        .height(Dimension::Pct(100.0))
        .background(fill)
        .rounded(ExtraRadius::Meter.role()),
    )
    .into()
}
