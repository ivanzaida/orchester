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
/// labels, in that order (state R, `VTnmr`), and below which a reached CLI's compact entry drops
/// its reset or reason (it stays in the hint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusBarWidths {
  pub version: f32,
  pub tokens: f32,
  pub labels: f32,
  pub details: f32,
}

/// The window width below which the waiting approvals' count reads *2 waiting* instead of *2 approvals
/// waiting*: at 960 the long form does not fit beside *Auto-approving* and Stop with the right group
/// whole; the hand and *Review* still say what waits (built, not drawn).
pub const APPROVALS_BRIEF_BELOW: f32 = 1120.0;

/// With the plan limits item showing percentages (about 170px). The activity then keeps at
/// least the room it has at 960 (`VTnmr`), which every other class keeps too.
pub const STATUS_WIDTHS: StatusBarWidths = StatusBarWidths {
  version: 1280.0,
  tokens: 1120.0,
  labels: 960.0,
  details: 0.0,
};

/// With one reached or refused CLI, whose entry names the limit and its reset or reason (about
/// 160px wider): everything goes 160px earlier.
pub const STATUS_WIDTHS_ONE_REACHED: StatusBarWidths = StatusBarWidths {
  version: 1440.0,
  tokens: 1280.0,
  labels: 1120.0,
  details: 0.0,
};

/// With both CLIs reached or refused (about 320px wider): everything goes 320px earlier, and
/// below 1120, where nothing else is left to drop, the entries drop their resets and reasons.
pub const STATUS_WIDTHS_BOTH_REACHED: StatusBarWidths = StatusBarWidths {
  version: 1600.0,
  tokens: 1440.0,
  labels: 1280.0,
  details: 1120.0,
};
