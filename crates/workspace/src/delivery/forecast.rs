//! When open work is likely delivered (ORC-50): a Monte Carlo over the project's recent daily throughput. Each trial
//! draws, day after day from tomorrow on, one of the days of the history at random and takes what was delivered that
//! day off the remaining work, until none is left; the 50% and 85% dates are the days by which half and 85 in a
//! hundred of the trials were done. The history is the project's last [`HISTORY_DAYS`] whole days before today, not
//! before its first task was made; with fewer than [`MIN_HISTORY_DAYS`] of them or fewer than [`MIN_DELIVERED`] tasks
//! delivered in them there is no forecast, only how much history there is. Work is counted in tasks, the unit of the
//! modules' burndown. Nothing here is money (`docs/architecture/native-agent.md` decision 8).

use chrono::{Days, NaiveDate};

/// The most whole days before today the history samples.
pub const HISTORY_DAYS: u64 = 30;
/// The fewest days of history a forecast rests on.
pub const MIN_HISTORY_DAYS: u64 = 10;
/// The fewest tasks delivered in the history a forecast rests on.
pub const MIN_DELIVERED: u32 = 3;
/// How many trials a forecast runs.
pub const TRIALS: usize = 2_000;
/// The farthest a trial looks ahead: ten years. Work a trial does not finish by then counts as done on that day.
pub const HORIZON_DAYS: u64 = 3_650;
/// The seed of the app's forecasts: the same history and work always give the same dates.
pub const SEED: u64 = 0x4F52_435F_3530_4D43;

/// The days a forecast samples: what was delivered on each of them, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
  delivered: Vec<u32>,
}

impl History {
  /// `delivered` per whole day, oldest first.
  pub const fn new(delivered: Vec<u32>) -> Self {
    Self { delivered }
  }

  /// How many days it has.
  pub fn days(&self) -> u64 {
    u64::try_from(self.delivered.len()).unwrap_or(u64::MAX)
  }

  /// How many tasks were delivered in it.
  pub fn delivered(&self) -> u32 {
    self.delivered.iter().sum()
  }

  /// Enough to forecast from.
  pub fn enough(&self) -> bool {
    self.days() >= MIN_HISTORY_DAYS && self.delivered() >= MIN_DELIVERED
  }

  /// The history of `today`: the whole days from [`HISTORY_DAYS`] before it, not before `first`, through
  /// yesterday, with what `delivered_on` each of them.
  pub fn before(today: NaiveDate, first: NaiveDate, delivered_on: impl Fn(NaiveDate) -> u32) -> Self {
    let start = today
      .checked_sub_days(Days::new(HISTORY_DAYS))
      .unwrap_or(today)
      .max(first);
    Self::new(
      start
        .iter_days()
        .take_while(|date| *date < today)
        .map(delivered_on)
        .collect(),
    )
  }
}

/// What a forecast says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outlook {
  /// No work is left.
  Done,
  /// The history is too short or too little was delivered in it: no dates.
  NotEnoughHistory,
  /// Half of the trials were done by `likely` and 85 in a hundred by `safe`; `past_horizon` when some of those
  /// trials ran to the end of [`HORIZON_DAYS`].
  Dates {
    likely: NaiveDate,
    safe: NaiveDate,
    past_horizon: bool,
  },
}

/// The forecast of `remaining` tasks, with the history it rests on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forecast {
  pub remaining: u32,
  pub history_days: u64,
  pub history_delivered: u32,
  pub outlook: Outlook,
}

impl Forecast {
  /// When `remaining` tasks are likely delivered after `today`, by `history`, with trials drawn from `seed`.
  pub fn of(remaining: u32, history: &History, today: NaiveDate, seed: u64) -> Self {
    let outlook = if remaining == 0 {
      Outlook::Done
    } else if !history.enough() {
      Outlook::NotEnoughHistory
    } else {
      dates(remaining, history, today, seed)
    };
    Self {
      remaining,
      history_days: history.days(),
      history_delivered: history.delivered(),
      outlook,
    }
  }
}

fn dates(remaining: u32, history: &History, today: NaiveDate, seed: u64) -> Outlook {
  let mut random = SplitMix(seed);
  let mut trials: Vec<u64> = (0..TRIALS)
    .map(|_| trial(remaining, &history.delivered, &mut random))
    .collect();
  trials.sort_unstable();
  let at = |share: usize| trials[(TRIALS * share).div_ceil(100) - 1];
  let (likely, safe) = (at(50), at(85));
  let day = |days: u64| today.checked_add_days(Days::new(days)).unwrap_or(NaiveDate::MAX);
  Outlook::Dates {
    likely: day(likely),
    safe: day(safe),
    past_horizon: safe >= HORIZON_DAYS,
  }
}

/// The days one trial takes to deliver `remaining` tasks, drawing a day of `delivered` for each.
fn trial(remaining: u32, delivered: &[u32], random: &mut SplitMix) -> u64 {
  let mut left = remaining;
  let mut days = 0;
  while left > 0 && days < HORIZON_DAYS {
    days += 1;
    left = left.saturating_sub(delivered[random.below(delivered.len())]);
  }
  days
}

/// SplitMix64: a small, fast generator whose draws depend only on its seed, so a forecast is the same each time.
struct SplitMix(u64);

impl SplitMix {
  fn next(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut mixed = self.0;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
  }

  /// A draw below `count`, which is not zero.
  fn below(&mut self, count: usize) -> usize {
    let count = u64::try_from(count).expect("a history fits in u64");
    usize::try_from(self.next() % count).expect("a draw below a usize fits in one")
  }
}
