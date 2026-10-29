//! *Plan limits*: only the accounts whose reading says they are near or at a limit now, and those whose reading said
//! so but whose window reset since, which show greyed as *unknown since reset*: their use now is not known and never
//! estimated. Every other account stays off the page (the sidebar and Settings › Accounts show them).

use orchester_core::{
  connection::plan_limits::{LimitReading, LimitState, LimitWindow, PlanReadings},
  id::AgentAccountId,
  time::Timestamp,
};

/// How an account near a limit shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitLook {
  Near,
  Reached,
  /// The reading said near or reached, and its window reset since.
  ResetSince,
}

/// An account near or at a limit, with the reading and the window that decides it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearLimit {
  pub account: AgentAccountId,
  pub look: LimitLook,
  pub reading: LimitReading,
  /// The window that decides it; `None` when a block no reset lifts decides it.
  pub window: Option<LimitWindow>,
}

/// The `accounts`, in their order, whose reading in `readings` is near or at a limit at `now`, or was when read.
pub fn near_limits(accounts: &[AgentAccountId], readings: &PlanReadings, now: Timestamp) -> Vec<NearLimit> {
  accounts
    .iter()
    .filter_map(|account| {
      let reading = readings.of_account(*account)?;
      let (look, at) = match reading.state(now) {
        LimitState::Near => (LimitLook::Near, now),
        LimitState::Reached => (LimitLook::Reached, now),
        LimitState::Normal if reading.state(reading.at) != LimitState::Normal => (LimitLook::ResetSince, reading.at),
        LimitState::Normal => return None,
      };
      Some(NearLimit {
        account: *account,
        look,
        window: reading.limiting(at).cloned(),
        reading: reading.clone(),
      })
    })
    .collect()
}
