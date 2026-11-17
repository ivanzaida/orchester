//! A team run's limits raised by its holder (OA-34; D6, the limits' owner, decided by the implementing agent on
//! 2026-11-17 by the option OA-34 proposes, to confirm with the person). A workspace's Admin sets a ceiling in its
//! policy ([`LimitRaises`], off by default): how many times one run of a task may be raised, and by how much at most
//! each time, the same ceiling the Orchestrator keeps for a local run it leads (OA-09: one raise by at most half). The
//! holder of the task's claim then asks the team server with `work.raise_limits` ([`LimitRaise`]) before its computer
//! raises the run; the server checks the ceiling, counts the raise and keeps it. With the ceiling off, a raise is
//! `decision_required [D6]` as before: the limits stay the Admin's.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most raises of one run a ceiling allows.
pub const MAX_RAISES_PER_RUN: u8 = 3;
/// The largest raise a ceiling allows, in percent of the limit raised.
pub const MAX_RAISE_PERCENT: u8 = 50;

/// The Admin's ceiling on raises of a team run's limits: at most `per_run` raises of one run, each limit by at most
/// `percent` % of what it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LimitRaisesWire", into = "LimitRaisesWire")]
pub struct LimitRaises {
  per_run: u8,
  percent: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitRaisesWire {
  per_run: u8,
  percent: u8,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LimitRaisesError {
  #[error("a run is raised 1 to {MAX_RAISES_PER_RUN} times")]
  PerRun,
  #[error("a raise is 1 to {MAX_RAISE_PERCENT} percent")]
  Percent,
}

/// Why a raise is beyond the ceiling.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RaiseRefused {
  /// The run was raised as many times as the ceiling allows.
  #[error("the run was raised {allowed} times already")]
  Raises { allowed: u8 },
  /// A limit would go beyond `most`.
  #[error("a limit may be raised to {most} at most")]
  By { most: u64 },
}

impl LimitRaises {
  pub const fn new(per_run: u8, percent: u8) -> Result<Self, LimitRaisesError> {
    if per_run == 0 || per_run > MAX_RAISES_PER_RUN {
      return Err(LimitRaisesError::PerRun);
    }
    if percent == 0 || percent > MAX_RAISE_PERCENT {
      return Err(LimitRaisesError::Percent);
    }
    Ok(Self { per_run, percent })
  }

  pub const fn per_run(self) -> u8 {
    self.per_run
  }

  pub const fn percent(self) -> u8 {
    self.percent
  }

  /// Whether a run raised `raised` times already may take `raise`: one raise more than the ceiling allows, or a limit
  /// raised by more than its share, is refused.
  pub fn allows(self, raised: u32, raise: &LimitRaise) -> Result<(), RaiseRefused> {
    if raised >= u32::from(self.per_run) {
      return Err(RaiseRefused::Raises { allowed: self.per_run });
    }
    for limit in raise.limits() {
      let most = limit
        .from
        .saturating_add(limit.from.saturating_mul(u64::from(self.percent)) / 100);
      if limit.to > most {
        return Err(RaiseRefused::By { most });
      }
    }
    Ok(())
  }
}

impl TryFrom<LimitRaisesWire> for LimitRaises {
  type Error = LimitRaisesError;

  fn try_from(wire: LimitRaisesWire) -> Result<Self, Self::Error> {
    Self::new(wire.per_run, wire.percent)
  }
}

impl From<LimitRaises> for LimitRaisesWire {
  fn from(value: LimitRaises) -> Self {
    Self {
      per_run: value.per_run,
      percent: value.percent,
    }
  }
}

/// One limit raised: from what the run had to what it gets, in the limit's unit (tokens, or milliseconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RaisedLimitWire", into = "RaisedLimitWire")]
pub struct RaisedLimit {
  from: u64,
  to: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RaisedLimitWire {
  from: u64,
  to: u64,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LimitRaiseError {
  /// A raise goes above what the run had, which is some limit: lifting a limit, or adding one, is no raise.
  #[error("a raise goes above a limit the run had")]
  NotAbove,
  /// A raise names at least one limit.
  #[error("a raise names a limit")]
  Empty,
}

impl RaisedLimit {
  pub const fn new(from: u64, to: u64) -> Result<Self, LimitRaiseError> {
    if from == 0 || to <= from {
      return Err(LimitRaiseError::NotAbove);
    }
    Ok(Self { from, to })
  }

  pub const fn from(self) -> u64 {
    self.from
  }

  pub const fn to(self) -> u64 {
    self.to
  }
}

impl TryFrom<RaisedLimitWire> for RaisedLimit {
  type Error = LimitRaiseError;

  fn try_from(wire: RaisedLimitWire) -> Result<Self, Self::Error> {
    Self::new(wire.from, wire.to)
  }
}

impl From<RaisedLimit> for RaisedLimitWire {
  fn from(value: RaisedLimit) -> Self {
    Self {
      from: value.from,
      to: value.to,
    }
  }
}

/// `{budget_tokens?, max_duration_ms?, step_budget_tokens?}`: one raise of a run stopped at its limit, each limit it
/// changes; a limit left out stays as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LimitRaiseWire", into = "LimitRaiseWire")]
pub struct LimitRaise {
  budget_tokens: Option<RaisedLimit>,
  max_duration_ms: Option<RaisedLimit>,
  step_budget_tokens: Option<RaisedLimit>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitRaiseWire {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  budget_tokens: Option<RaisedLimit>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  max_duration_ms: Option<RaisedLimit>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  step_budget_tokens: Option<RaisedLimit>,
}

impl LimitRaise {
  /// The run's token budget and time limit raised, or the budget of the step it stopped at.
  pub const fn new(
    budget_tokens: Option<RaisedLimit>,
    max_duration_ms: Option<RaisedLimit>,
    step_budget_tokens: Option<RaisedLimit>,
  ) -> Result<Self, LimitRaiseError> {
    if budget_tokens.is_none() && max_duration_ms.is_none() && step_budget_tokens.is_none() {
      return Err(LimitRaiseError::Empty);
    }
    Ok(Self {
      budget_tokens,
      max_duration_ms,
      step_budget_tokens,
    })
  }

  pub const fn budget_tokens(self) -> Option<RaisedLimit> {
    self.budget_tokens
  }

  pub const fn max_duration_ms(self) -> Option<RaisedLimit> {
    self.max_duration_ms
  }

  pub const fn step_budget_tokens(self) -> Option<RaisedLimit> {
    self.step_budget_tokens
  }

  /// Every limit the raise changes.
  pub fn limits(&self) -> impl Iterator<Item = RaisedLimit> {
    [self.budget_tokens, self.max_duration_ms, self.step_budget_tokens]
      .into_iter()
      .flatten()
  }
}

impl TryFrom<LimitRaiseWire> for LimitRaise {
  type Error = LimitRaiseError;

  fn try_from(wire: LimitRaiseWire) -> Result<Self, Self::Error> {
    Self::new(wire.budget_tokens, wire.max_duration_ms, wire.step_budget_tokens)
  }
}

impl From<LimitRaise> for LimitRaiseWire {
  fn from(value: LimitRaise) -> Self {
    Self {
      budget_tokens: value.budget_tokens,
      max_duration_ms: value.max_duration_ms,
      step_budget_tokens: value.step_budget_tokens,
    }
  }
}
