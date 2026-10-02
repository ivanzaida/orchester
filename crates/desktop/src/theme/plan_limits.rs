//! Plan-limit hint geometry (`mFTm5`, `pNIEg`, `DCH2x` in `poPso`) and the narrow status bar
//! (state R, `VTnmr`), which drops the version, then Tokens today, then the count labels, never
//! the plan limits.
pub const PLAN_HINT_WIDTH: f32 = 360.0;
pub const PLAN_WINDOW_LABEL: f32 = 44.0;
pub const PLAN_METER_WIDTH: f32 = 100.0;
pub const PLAN_METER_HEIGHT: f32 = 4.0;
/// The used percentage's column, right-aligned so the resets line up (`KDuKw`, `Y1XNS`).
pub const PLAN_PERCENT_WIDTH: f32 = 32.0;

/// The window widths below which the status bar drops the version, Tokens today and the count
/// labels, in that order (state R, `VTnmr`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusBarWidths {
  pub version: f32,
  pub tokens: f32,
  pub labels: f32,
}

/// With the plan limits item showing percentages (about 170px).
pub const STATUS_WIDTHS: StatusBarWidths = StatusBarWidths {
  version: 1280.0,
  tokens: 1120.0,
  labels: 960.0,
};

/// With a reached or refused CLI, whose item names the limit and its reset or reason (about
/// 300–320px): everything goes 80px earlier, the count labels already below the tokens' width.
pub const STATUS_WIDTHS_REACHED: StatusBarWidths = StatusBarWidths {
  version: 1360.0,
  tokens: 1200.0,
  labels: 1120.0,
};
