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

/// The window widths from which the waiting approvals in the status bar show each of their parts whole
/// (built, not drawn): *2 approvals waiting* rather than *2 waiting*, the time, which run auto-approves
/// beside them, and what the oldest request asks (with room for at least 100px of it). Below a width a
/// part does not show at all. Measured headlessly on 2026-10-03 for the worst case of each plan limits
/// class (a reached or refused CLI widens the right group), a five-digit MCP port, Tokens today with an
/// 11-character value and two-digit counts, run 1234 of ORC-1234, at every 10px from 500 to 2200;
/// each width is the lowest from which the part fits at every wider width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApprovalsWidths {
  pub long_count: f32,
  pub time: f32,
  pub run: f32,
  pub asks: f32,
}

/// Without a run that auto-approves, by how many CLIs are reached or refused (0, 1, 2 or more).
const APPROVALS_WIDTHS: [ApprovalsWidths; 3] = [
  ApprovalsWidths {
    long_count: 0.0,
    time: 1150.0,
    run: 0.0,
    asks: 1310.0,
  },
  ApprovalsWidths {
    long_count: 0.0,
    time: 1310.0,
    run: 0.0,
    asks: 1470.0,
  },
  ApprovalsWidths {
    long_count: 0.0,
    time: 1470.0,
    run: 0.0,
    asks: 1620.0,
  },
];

/// Beside *Auto-approving* and Stop, by how many CLIs are reached or refused.
const APPROVALS_WIDTHS_AUTO: [ApprovalsWidths; 3] = [
  ApprovalsWidths {
    long_count: 1310.0,
    time: 1360.0,
    run: 1490.0,
    asks: 1590.0,
  },
  ApprovalsWidths {
    long_count: 1470.0,
    time: 1510.0,
    run: 1640.0,
    asks: 1740.0,
  },
  ApprovalsWidths {
    long_count: 1620.0,
    time: 1670.0,
    run: 1800.0,
    asks: 1900.0,
  },
];

/// The widths of the waiting approvals' parts with `reached` CLIs reached or refused, beside a run
/// that auto-approves or not.
pub fn approvals_widths(reached: usize, auto_approving: bool) -> ApprovalsWidths {
  let class = reached.min(2);
  if auto_approving {
    APPROVALS_WIDTHS_AUTO[class]
  } else {
    APPROVALS_WIDTHS[class]
  }
}

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
