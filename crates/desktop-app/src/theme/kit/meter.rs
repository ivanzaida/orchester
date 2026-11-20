//! Meter (`Meter` `wtAQI`, `ui::meter`): lurq's `ui::Progress` as a 4 high `border` track with the used part filled
//! from its start, both radius 2, as wide as its parent and with no words. The fill's color is the tone's variant.

use lurq::{
  app::theme::{PaletteColor, Theme},
  ui::{Length, Progress, ProgressPart},
};

use crate::theme::{ExtraColor, ExtraRadius};

/// The track's height (`wtAQI`).
const HEIGHT: f32 = 4.0;

/// The variants a meter's tone names, each its fill's color.
pub const PLAIN: &str = "plain";
pub const REACHED: &str = "reached";
pub const PROGRESS: &str = "progress";
pub const QUIET: &str = "quiet";
pub const FULL: &str = "full";

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Progress>(|rules| {
    rules
      .base(ProgressPart::Root, |style| style.width(Length::Fill))
      .base(ProgressPart::Track, |style| {
        style
          .height(HEIGHT)
          .radius(ExtraRadius::Meter.role())
          .background(PaletteColor::Border)
      })
      .base(ProgressPart::Fill, |style| style.radius(ExtraRadius::Meter.role()))
      .variant(PLAIN, ProgressPart::Fill, |style| {
        style.background(ExtraColor::TextSecondary.role())
      })
      .variant(REACHED, ProgressPart::Fill, |style| {
        style.background(PaletteColor::Warning)
      })
      .variant(PROGRESS, ProgressPart::Fill, |style| {
        style.background(PaletteColor::Info)
      })
      .variant(QUIET, ProgressPart::Fill, |style| {
        style.background(PaletteColor::MutedForeground)
      })
      .variant(FULL, ProgressPart::Fill, |style| {
        style.background(PaletteColor::Destructive)
      })
  });
}
