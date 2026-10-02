//! Orchester's lurq theme, built from the Pencil design variables.
//!
//! UI code styles through lurq roles (`PaletteColor`, `TypographyStyle`, `SpacingSize`,
//! `RadiusSize`, `BorderSize`, `ShadowStyle`); design values live only in `tokens`.

mod canvas_grid;
mod form;
mod markdown;
mod metrics;
mod palette;
mod plan_limits;
pub use plan_limits::{
  PLAN_HINT_WIDTH, PLAN_METER_HEIGHT, PLAN_METER_WIDTH, PLAN_WINDOW_LABEL, STATUS_LABELS_MIN_WIDTH,
  STATUS_TOKENS_MIN_WIDTH, STATUS_VERSION_MIN_WIDTH,
};
mod shadow;
mod tokens;
mod typography;

pub use canvas_grid::{CANVAS_GRID_TILE, canvas_grid};
pub use form::{gutter_scrollbar, gutter_width, select_menu_scrollbar};
use lurq::app::App;
pub use metrics::{ExtraRadius, ExtraSpacing};
pub use palette::ExtraColor;
pub use shadow::ExtraShadow;
pub use typography::ExtraTypography;

/// Height of the custom title bar drawn above every screen (`QQmsI`, `V70DN`).
pub const TITLE_BAR_HEIGHT: f32 = tokens::TITLE_BAR_HEIGHT;
/// Height of the status bar drawn below every screen (`pmOpt`).
pub const STATUS_BAR_HEIGHT: f32 = tokens::STATUS_BAR_HEIGHT;
/// Width of each Windows window control (`QQmsI`).
pub const WINDOW_CONTROL_WIDTH: f32 = tokens::WINDOW_CONTROL_WIDTH;
/// Width of the lifecycle dialogs at most (`um9B1`, `KS2RU`, `BovYZ`); narrower windows keep
/// `ExtraSpacing::DialogMargin` on either side instead.
pub const LIFECYCLE_DIALOG_WIDTH: f32 = tokens::LIFECYCLE_DIALOG_WIDTH;
/// Stroke of the pipeline canvas connectors (`zpizf`, `gXNw9`).
pub const CONNECTOR_STROKE: f32 = tokens::CONNECTOR_STROKE;
/// Width of the check slot every select option reserves, the size of the check (`ClxwV` in
/// `L20XvS`).
pub const SELECT_CHECK_SLOT: f32 = tokens::SELECT_ICON_SIZE;
/// Opacity of unparsed CLI lines in the agent transcript (`Ane9j` in `SfXU1`).
pub const UNPARSED_OPACITY: f32 = tokens::UNPARSED_OPACITY;
/// Opacity of an option that cannot be chosen (`BNliD` in `T1rjv`).
pub const UNAVAILABLE_OPTION_OPACITY: f32 = tokens::UNAVAILABLE_OPTION_OPACITY;
/// Opacity of a button that cannot be pressed, over its variant's own colors (`kLJJb`).
pub const DISABLED_OPACITY: f32 = tokens::DISABLED_OPACITY;
/// Opacity of a switch whose value is fixed, track and label as one group (`lW7Sp`).
pub const LOCKED_SWITCH_OPACITY: f32 = tokens::LOCKED_SWITCH_OPACITY;

const FONTS: &[&[u8]] = &[
  include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
  include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
  include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
  include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
  include_bytes!("../../assets/fonts/GeistMono-Regular.ttf"),
  // Lucide icons (ISC license), the same font lurq bundles for its DevTools.
  include_bytes!("../../assets/fonts/lucide.ttf"),
];

pub fn install(app: &mut App) {
  for font in FONTS {
    app.load_font(font.to_vec());
  }

  let theme = app.theme();
  theme.set_palette(palette::palette());
  theme.set_typography(typography::typography());
  theme.set_radii(metrics::radii());
  theme.set_spacing(metrics::spacing());
  theme.set_border_sizes(metrics::border_sizes());
  theme.set_shadows(shadow::shadows());
  theme.set_markdown(markdown::markdown());
  // Bind first: the guard returned by `form()` must be released before `set_form` locks.
  let current_form = theme.form().clone();
  theme.set_form(form::form(current_form));
}
