//! Plan-limit hint geometry (`mFTm5`, `pNIEg`, `DCH2x` in `poPso`) and the narrow status bar
//! (state R, `VTnmr`), which drops the version, then Tokens today, then the count labels, never
//! the plan limits.
pub const PLAN_HINT_WIDTH: f32 = 360.0;
pub const PLAN_WINDOW_LABEL: f32 = 44.0;
pub const PLAN_METER_WIDTH: f32 = 100.0;
pub const PLAN_METER_HEIGHT: f32 = 4.0;
/// Below this window width the status bar drops the version.
pub const STATUS_VERSION_MIN_WIDTH: f32 = 1280.0;
/// Below this width it drops Tokens today, and the count labels while a plan limit is reached.
pub const STATUS_TOKENS_MIN_WIDTH: f32 = 1120.0;
/// Below this width it drops the count labels.
pub const STATUS_LABELS_MIN_WIDTH: f32 = 960.0;
