//! Which plan limit windows the page draws (by day, 5-hour windows would be a comb of spikes and are left
//! out), and which readings its table lists (`Table` `CFzvr` in `d3lc3`).

use orchester_core::usage::{
  Grouping,
  limits::{LimitLine, LimitPoint, WindowKind},
};

use super::Dashboard;

/// The readings the table lists: it reads beside *Tokens over time*, and the chart has them all.
pub const TABLE_READINGS: usize = 7;

impl Dashboard {
  /// The windows the chart draws: 5-hour windows only by hour.
  pub fn drawn_limits(&self) -> Vec<&LimitLine> {
    self
      .limits
      .lines
      .iter()
      .filter(|line| self.grouping == Grouping::Hour || line.kind() != WindowKind::FiveHour)
      .collect()
  }

  /// The table's readings, newest first: the [`TABLE_READINGS`] newest of the drawn windows' readings whose
  /// rounded % differs from the reading before it of the same window, so the rows show where use moved.
  pub fn limit_table(&self) -> Vec<(&LimitLine, &LimitPoint)> {
    let mut readings: Vec<(&LimitLine, &LimitPoint)> = self
      .drawn_limits()
      .into_iter()
      .flat_map(|line| {
        let mut before = None;
        line.points.iter().filter_map(move |point| {
          let used = point.percent.round();
          let moved = before != Some(used);
          before = Some(used);
          moved.then_some((line, point))
        })
      })
      .collect();
    readings.sort_by_key(|(_, point)| std::cmp::Reverse(point.at));
    readings.truncate(TABLE_READINGS);
    readings
  }
}
