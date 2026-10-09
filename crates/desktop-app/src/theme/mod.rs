//! Orchester's lurq theme, from the variables and type roles of `data/orchester2.pen` (Theme board
//! `w3uvVL`), with the `graphite` accent in dark and light mode.
//!
//! UI code styles only through lurq roles (`PaletteColor`, `TypographyStyle`, `SpacingSize`,
//! `RadiusSize`, `BorderSize`, `ShadowStyle`) and the extra roles defined here; design values live only
//! in this module.
//!
//! The roles are the design's complete token set on purpose, also where no screen uses one yet (the
//! selection, scrim, conversation, code and diff colors, `display` type, `radius_full`): the screens
//! that need them come next and must find them here rather than add near-duplicates.

mod code;
mod fonts;
mod form;
mod markdown;
mod metrics;
mod palette;
mod shadow;
mod shell;
mod startup;
mod task;
mod tasks;
mod typography;

use std::time::Duration;

use lurq::{
  app::{App, theme::Theme},
  layout::scrollbar::ScrollBarStyle,
};

pub use self::{
  code::{CodeText, token_color},
  metrics::{ControlHeight, ExtraRadius, ExtraSpacing},
  palette::{ExtraColor, Mode},
  shadow::ExtraShadow,
  shell::{ShellSpacing, ShellText},
  startup::{StartupSpacing, StartupText},
  task::{TaskSpacing, TaskText},
  tasks::TasksSpacing,
  typography::{ExtraTypography, IconSize},
};

/// A control that cannot be used, faded as a whole: a button that is unavailable or disabled (`Create
/// multiple` `Q9aeZ`, `Attach context` `p6caAv`).
pub const DISABLED_OPACITY: f32 = 0.5;
/// A shortcut beside a tooltip's label (`n75PTn` in `b6q3OZ`).
pub const SHORTCUT_OPACITY: f32 = 0.6;
/// One turn of a loader (`loader-circle`), which turns while it is shown; the design draws it still.
pub const LOADER_TURN: Duration = Duration::from_secs(1);

/// Loads the fonts and fills every role for `mode`.
pub fn install(app: &mut App, mode: Mode) {
  fonts::install(app);
  let theme = app.theme();
  apply(theme, mode);
  metrics::install(theme);
  startup::install(theme);
  task::install(theme);
  tasks::install(theme);
  shell::install(theme);
  theme.set_shadows(shadow::shadows());
  theme.set_form(form::form());
}

/// Switches the colors to `mode`: the palette, and the text styles, scrollbar and Markdown whose colors
/// come from it. Every component that reads the theme follows on its next render.
pub fn apply(theme: &Theme, mode: Mode) {
  let palette = palette::palette(mode);
  theme.set_typography(typography::typography(&palette));
  theme.set_scrollbar(ScrollBarStyle {
    thumb_color: palette.border_strong,
    ..ScrollBarStyle::thin()
  });
  theme.set_markdown(markdown::markdown(&palette));
  theme.set_palette(palette);
}
