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
/// Backdrop behind the close dialog (`a8U756`).
pub const SCRIM: &str = "#000000A6";

/// Code and diff colors (Pencil `code-*` and `diff-*`, used by the diff line `vEZ42`): a
/// restrained GitHub Dark palette for code only; the application chrome stays grayscale.
pub const CODE_BG: &str = "#0D1117";
pub const CODE_TEXT: &str = "#E6EDF3";
pub const CODE_KEYWORD: &str = "#FF7B72";
pub const CODE_STRING: &str = "#A5D6FF";
pub const CODE_FUNCTION: &str = "#D2A8FF";
pub const CODE_VARIABLE: &str = "#FFA657";
pub const CODE_CONSTANT: &str = "#79C0FF";
pub const CODE_COMMENT: &str = "#8B949E";
pub const DIFF_ADD_BG: &str = "#14261C";
pub const DIFF_ADD_TEXT: &str = "#7EE787";
pub const DIFF_REMOVE_BG: &str = "#301C20";
pub const DIFF_REMOVE_TEXT: &str = "#FFA198";

pub const FONT: &str = "Inter";
/// Pencil's `mono`, bundled from `assets/fonts/GeistMono-Regular.ttf` (SIL OFL 1.1).
pub const MONO: &str = "Geist Mono";

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
/// Underline of the selected tab (`Iefue`, `Okyxt`).
pub const TAB_INDICATOR_STROKE: f32 = 2.0;
/// Chevron of a select field and check of its selected option (`eASPF` in `ZGwlv`, `ClxwV` in
/// `L20XvS`); also the width of the check slot every option reserves.
pub const SELECT_ICON_SIZE: f32 = 14.0;
/// Lucide icon font, the icon library of every Pencil icon node.
pub const ICON_FONT: &str = "lucide";

/// Corner radius of inline notices and the runner status card (`q5LzI`, `O2Jjjf`).
pub const RADIUS_NOTICE: f32 = 8.0;
/// Corner radius of the bars in the Orchester mark (`DIMZ6`).
pub const RADIUS_MARK: f32 = 2.0;
/// Corner radius of an open select menu: option radius 7 plus its padding 4 (`u0ByM`).
pub const RADIUS_SELECT_MENU: f32 = 11.0;
/// Switch track (`K8ItM`, radius 8 on a 16px track). lurq clamps a radius to half the box, so
/// the 10px knob is round with it too.
pub const RADIUS_PILL: f32 = 8.0;

/// Gaps and paddings used by the Getting started screens, the sidebar and the overview.
/// Pencil node ids are listed on the `ExtraSpacing` variants that use them.
pub const SPACE_HAIR: f32 = 2.0;
pub const SPACE_MARK: f32 = 3.0;
pub const SPACE_TIGHT: f32 = 6.0;
pub const SPACE_INDICATOR: f32 = 7.0;
pub const SPACE_INLINE: f32 = 10.0;
pub const SPACE_STACK: f32 = 12.0;
pub const SPACE_GROUP: f32 = 14.0;
pub const SPACE_STEP: f32 = 20.0;
pub const SPACE_RAIL: f32 = 28.0;
pub const SPACE_HEADING_TOP: f32 = 26.0;
pub const SPACE_HEADING_BOTTOM: f32 = 22.0;
pub const SPACE_FORM: f32 = 22.0;
pub const SPACE_SECTION: f32 = 32.0;
pub const SPACE_INDENT: f32 = 40.0;
pub const SPACE_PAGE: f32 = 48.0;
pub const SPACE_HERO: f32 = 64.0;
pub const SPACE_FOOTER: f32 = 72.0;
pub const SPACE_COLUMNS: f32 = 112.0;
pub const SPACE_MARGIN: f32 = 120.0;
pub const SPACE_FIELD: f32 = 9.0;
pub const SPACE_PROPERTY: f32 = 15.0;
pub const SPACE_DIVIDER: f32 = 18.0;
pub const SPACE_SELECT_MENU: f32 = 4.0;

/// Stroke of the pipeline canvas connectors (`zpizf`, `gXNw9`).
pub const CONNECTOR_STROKE: f32 = 1.25;

/// Custom window title bar height (`QQmsI`, `V70DN`).
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
/// Width of each Windows window control (`QQmsI`).
pub const WINDOW_CONTROL_WIDTH: f32 = 46.0;

/// Drop shadow of the status bar operations popover (`E1msJS`): outer, x 0, y 8, blur 24,
/// no spread. The only shadow in the design.
pub const POPOVER_SHADOW_Y: f32 = 8.0;
pub const POPOVER_SHADOW_BLUR: f32 = 24.0;
pub const POPOVER_SHADOW_COLOR: &str = "#00000066";

/// Focus ring (`xURvJ`, anatomy `Oz9EN`): a 2px light ring behind a 2px dark gap, drawn as
/// two spread box shadows (Pencil `focus-ring`, `focus-gap`, `focus-width`, `focus-extent`).
pub const FOCUS_RING: &str = "#EEEEEE";
pub const FOCUS_GAP: &str = "#101010";
pub const FOCUS_WIDTH: f32 = 2.0;
pub const FOCUS_EXTENT: f32 = 4.0;

/// Overlay scrollbar of a long select menu (`g3poI`): a 4px `border` thumb, radius 2, no track.
pub const SELECT_SCROLLBAR_WIDTH: f32 = 4.0;
pub const SELECT_SCROLLBAR_RADIUS: f32 = 2.0;
/// Scroll thumb of the select menu (`YD8PV`): 3px from the menu's outer right edge and 11px (the
/// corner radius) from its top and bottom, so it never enters a rounded corner; at least 24px long.
pub const SELECT_SCROLLBAR_EDGE_INSET: f32 = 3.0;
pub const SELECT_SCROLLBAR_END_INSET: f32 = 11.0;
pub const SELECT_SCROLLBAR_MIN_THUMB: f32 = 24.0;
pub const NO_TRACK: &str = "#00000000";

/// Unparsed CLI lines in the agent transcript are muted to 60 % (`Ane9j` in `SfXU1`).
pub const UNPARSED_OPACITY: f32 = 0.6;
