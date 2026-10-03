//! Plan-limit hint geometry (`mFTm5`, `pNIEg`, `DCH2x` in `poPso`) and the least of a waiting
//! request's words the narrow status bar keeps.
pub const PLAN_HINT_WIDTH: f32 = 360.0;
pub const PLAN_WINDOW_LABEL: f32 = 44.0;
pub const PLAN_METER_WIDTH: f32 = 100.0;
pub const PLAN_METER_HEIGHT: f32 = 4.0;
/// The used percentage's column, right-aligned so the resets line up (`KDuKw`, `Y1XNS`).
pub const PLAN_PERCENT_WIDTH: f32 = 32.0;

/// The least of what a waiting request asks that the status bar shows, in px: when the bar is narrow
/// the request's words trim their end down to this, so they never become a stub (built, not drawn).
pub const APPROVALS_ASKS_MIN: f32 = 100.0;
