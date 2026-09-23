//! Design variables from `data/orchester.pen` (Pencil `get_variables`), plus values that
//! the Foundations components use directly. Visible only inside the theme module:
//! UI code reads lurq theme roles, never these constants.

pub const CANVAS: &str = "#101010";
pub const SIDEBAR: &str = "#141414";
pub const SURFACE: &str = "#1A1A1A";
pub const RAISED: &str = "#222222";
pub const HOVER: &str = "#2C2C2C";
pub const BORDER: &str = "#363636";
pub const HAIRLINE: &str = "#2B2B2B";
pub const TEXT: &str = "#EEEEEE";
pub const MUTED: &str = "#A3A3A3";
pub const SUBTLE: &str = "#8F8F8F";
pub const ACCENT: &str = "#E6E6E6";
pub const INVERSE_TEXT: &str = "#181818";

pub const FONT: &str = "Inter";
/// Pencil's `mono` is Geist Mono, which is not bundled yet; this is the system fallback.
pub const MONO: &str = "monospace";

pub const RADIUS_CONTROL: f32 = 7.0;
pub const RADIUS_PANEL: f32 = 12.0;
/// Navigation item corner radius (`TSJpn`).
pub const RADIUS_ITEM: f32 = 6.0;

pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
pub const SPACE_LG: f32 = 24.0;

/// Field and button heights and horizontal padding (`ZGwlv`, `u1XBP`).
pub const FIELD_HEIGHT: f32 = 40.0;
pub const BUTTON_HEIGHT: f32 = 36.0;
pub const CONTROL_PADDING_X: f32 = 12.0;
/// Focused field stroke (`o8Oug`).
pub const FOCUS_STROKE: f32 = 1.5;
