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
/// Lucide icon font, the icon library of every Pencil icon node.
pub const ICON_FONT: &str = "lucide";

/// Corner radius of inline notices and the runner status card (`q5LzI`, `O2Jjjf`).
pub const RADIUS_NOTICE: f32 = 8.0;
/// Corner radius of the bars in the Orchester mark (`DIMZ6`).
pub const RADIUS_MARK: f32 = 2.0;

/// Gaps and paddings used by the Getting started screens, the sidebar and the overview.
/// Pencil node ids are listed on the `ExtraSpacing` variants that use them.
pub const SPACE_MARK: f32 = 3.0;
pub const SPACE_INLINE: f32 = 10.0;
pub const SPACE_STACK: f32 = 12.0;
pub const SPACE_GROUP: f32 = 14.0;
pub const SPACE_STEP: f32 = 20.0;
pub const SPACE_RAIL: f32 = 28.0;
pub const SPACE_SECTION: f32 = 32.0;
pub const SPACE_INDENT: f32 = 40.0;
pub const SPACE_PAGE: f32 = 48.0;
pub const SPACE_HERO: f32 = 64.0;
pub const SPACE_FOOTER: f32 = 72.0;
pub const SPACE_COLUMNS: f32 = 112.0;
pub const SPACE_MARGIN: f32 = 120.0;

/// Custom window title bar height (`QQmsI`, `V70DN`).
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
