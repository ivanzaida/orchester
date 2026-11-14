//! How a subscription takes its target's work (`docs/plan/orchestrator-autonomy/OA-09-lead-mode-and-ceilings.md`): it
//! *follows* it, as every subscription did (events start turns, and the chat acts as the drive switch lets it), or
//! *leads it to the end*, which only the person chooses. What the chat does on its own for work it leads to the end is
//! bounded by [`Ceilings`], which its tools enforce in the turns Orchester starts by itself: how often a task is run,
//! how often the chat returns or retries a run, how often and by how much it raises a limit, and how many tokens the
//! subscription's work may spend. Past a ceiling the chat leaves the task to the person.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  run::{Answerer, RunEvent, RunEventKind, StuckDecision},
  tokens::{TokenBudget, TokensUsed},
};

/// How a subscription takes its target's work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WatchMode {
  /// Its events start turns in the conversation; nothing bounds what the chat does but the person's switch.
  #[default]
  Follow,
  /// The chat carries its work to the end, within `ceilings`.
  Lead { ceilings: Ceilings },
}

impl WatchMode {
  /// The ceilings of a subscription that leads to the end; `None` for one that follows.
  pub const fn ceilings(self) -> Option<Ceilings> {
    match self {
      Self::Lead { ceilings } => Some(ceilings),
      Self::Follow => None,
    }
  }
}

/// The words that, at the start of the order of the person's own /watch, have its targets led to the end: the way to
/// choose it until a window offers it (`docs/plan/orchestrator-autonomy/OA-10-…`).
pub const LEAD_TO_THE_END: &str = "lead to the end";

/// The rest of `order` when it begins with [`LEAD_TO_THE_END`], in any case, as a whole phrase: what follows it and
/// the punctuation after it is the order itself. `None` for any other order.
pub fn asked_to_lead(order: &str) -> Option<&str> {
  let trimmed = order.trim_start();
  let head = trimmed.get(..LEAD_TO_THE_END.len())?;
  if !head.eq_ignore_ascii_case(LEAD_TO_THE_END) {
    return None;
  }
  let rest = &trimmed[LEAD_TO_THE_END.len()..];
  // "lead to the endgame" is an order of its own.
  if rest.chars().next().is_some_and(char::is_alphanumeric) {
    return None;
  }
  Some(rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '.' | ',' | ';' | '!' | '-')))
}

/// What bounds the chat's own doing for work it leads to the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Kept", into = "Kept")]
pub struct Ceilings {
  runs_of_task: u8,
  returns_of_run: u8,
  raises_of_run: u8,
  raise_percent: u8,
  budget: Option<TokenBudget>,
}

/// The ceilings as they are kept.
#[derive(Clone, Copy, Serialize, Deserialize)]
struct Kept {
  runs_of_task: u8,
  returns_of_run: u8,
  raises_of_run: u8,
  raise_percent: u8,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  budget: Option<TokenBudget>,
}

/// A ceiling outside what it may be set to.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CeilingsError {
  #[error("a task is run from {min} to {max} times", min = Ceilings::MIN_RUNS, max = Ceilings::MAX_RUNS)]
  Runs,
  #[error("a run is returned or retried at most {max} times", max = Ceilings::MAX_RETURNS)]
  Returns,
  #[error("a run's limit is raised at most {max} times", max = Ceilings::MAX_RAISES)]
  Raises,
  #[error("a limit is raised by 1 to {max} percent", max = Ceilings::MAX_RAISE_PERCENT)]
  RaisePercent,
}

impl Ceilings {
  pub const MIN_RUNS: u8 = 1;
  pub const MAX_RUNS: u8 = 5;
  pub const MAX_RETURNS: u8 = 10;
  pub const MAX_RAISES: u8 = 3;
  pub const MAX_RAISE_PERCENT: u8 = 100;

  /// A task is run at most `runs_of_task` times, its first run included; the chat returns or retries one run at most
  /// `returns_of_run` times, and raises its limits at most `raises_of_run` times, each by at most `raise_percent`
  /// percent of the limit; the subscription's work spends at most `budget` tokens, or any number without one.
  pub const fn new(
    (runs_of_task, returns_of_run): (u8, u8),
    (raises_of_run, raise_percent): (u8, u8),
    budget: Option<TokenBudget>,
  ) -> Result<Self, CeilingsError> {
    if runs_of_task < Self::MIN_RUNS || runs_of_task > Self::MAX_RUNS {
      return Err(CeilingsError::Runs);
    }
    if returns_of_run > Self::MAX_RETURNS {
      return Err(CeilingsError::Returns);
    }
    if raises_of_run > Self::MAX_RAISES {
      return Err(CeilingsError::Raises);
    }
    if raise_percent == 0 || raise_percent > Self::MAX_RAISE_PERCENT {
      return Err(CeilingsError::RaisePercent);
    }
    Ok(Self {
      runs_of_task,
      returns_of_run,
      raises_of_run,
      raise_percent,
      budget,
    })
  }

  pub const fn runs_of_task(self) -> u8 {
    self.runs_of_task
  }

  pub const fn returns_of_run(self) -> u8 {
    self.returns_of_run
  }

  pub const fn raises_of_run(self) -> u8 {
    self.raises_of_run
  }

  pub const fn raise_percent(self) -> u8 {
    self.raise_percent
  }

  pub const fn budget(self) -> Option<TokenBudget> {
    self.budget
  }

  /// Whether a task that had `runs` runs may be run once more.
  pub fn another_run(self, runs: usize) -> Result<(), CeilingReached> {
    below(Ceiling::RunsOfTask, runs, self.runs_of_task)
  }

  /// Whether a run automation returned or retried `own` times may be returned or retried once more.
  pub fn another_return(self, own: usize) -> Result<(), CeilingReached> {
    below(Ceiling::ReturnsOfRun, own, self.returns_of_run)
  }

  /// Whether a run whose limits were raised `raises` times may have a limit raised from `from` to `to`, both in the
  /// limit's own unit (tokens, or minutes); `to` is `None` for no limit at all, which no ceiling allows.
  pub fn raise(self, raises: usize, from: u64, to: Option<u64>) -> Result<(), CeilingReached> {
    below(Ceiling::RaisesOfRun, raises, self.raises_of_run)?;
    let most = from.saturating_add(from.saturating_mul(u64::from(self.raise_percent)) / 100);
    match to {
      Some(to) if to <= most => Ok(()),
      to => Err(CeilingReached {
        ceiling: Ceiling::RaiseBy,
        allowed: most,
        used: to.unwrap_or(u64::MAX),
      }),
    }
  }

  /// Whether work that spent `used` tokens may start more.
  pub fn within_budget(self, used: TokensUsed) -> Result<(), CeilingReached> {
    match self.budget {
      Some(budget) if used.reached(budget) => Err(CeilingReached {
        ceiling: Ceiling::Budget,
        allowed: budget.get(),
        used: used.get(),
      }),
      Some(_) | None => Ok(()),
    }
  }
}

impl Default for Ceilings {
  /// Three runs of a task, four returns or retries of a run, one raise of a limit by at most half, and no budget
  /// (the person's decision of 2026-11-13).
  fn default() -> Self {
    Self {
      runs_of_task: 3,
      returns_of_run: 4,
      raises_of_run: 1,
      raise_percent: 50,
      budget: None,
    }
  }
}

impl TryFrom<Kept> for Ceilings {
  type Error = CeilingsError;

  fn try_from(kept: Kept) -> Result<Self, CeilingsError> {
    Self::new(
      (kept.runs_of_task, kept.returns_of_run),
      (kept.raises_of_run, kept.raise_percent),
      kept.budget,
    )
  }
}

impl From<Ceilings> for Kept {
  fn from(ceilings: Ceilings) -> Self {
    Self {
      runs_of_task: ceilings.runs_of_task,
      returns_of_run: ceilings.returns_of_run,
      raises_of_run: ceilings.raises_of_run,
      raise_percent: ceilings.raise_percent,
      budget: ceilings.budget,
    }
  }
}

/// One of a leading subscription's ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ceiling {
  /// The runs of a task.
  RunsOfTask,
  /// Automation's returns and retries of a run.
  ReturnsOfRun,
  /// The raises of a run's limits.
  RaisesOfRun,
  /// By how much a limit is raised.
  RaiseBy,
  /// The tokens the subscription's work spends.
  Budget,
}

/// A call would pass `ceiling`: `allowed` is the most it allows and `used` what there is already (for
/// [`Ceiling::RaiseBy`], the limit asked for).
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("the ceiling {ceiling:?} allows {allowed}, and {used} would pass it")]
pub struct CeilingReached {
  pub ceiling: Ceiling,
  pub allowed: u64,
  pub used: u64,
}

fn below(ceiling: Ceiling, used: usize, allowed: u8) -> Result<(), CeilingReached> {
  let used = u64::try_from(used).unwrap_or(u64::MAX);
  if used < u64::from(allowed) {
    return Ok(());
  }
  Err(CeilingReached {
    ceiling,
    allowed: u64::from(allowed),
    used,
  })
}

/// How often automation returned or retried the run of `events`: its retries and returns of a run that waited, its
/// returns of an ended run and its continuations of a failed one. A person's are theirs and do not count.
pub fn own_returns(events: &[RunEvent]) -> usize {
  events
    .iter()
    .filter(|event| match event.kind() {
      RunEventKind::StuckDecided { decision, by } => {
        *by == Answerer::Automation && matches!(decision, StuckDecision::Retry | StuckDecision::ReturnTo { .. })
      }
      RunEventKind::ReturnDecided { by, .. } | RunEventKind::RecoveryDecided { by, .. } => *by == Answerer::Automation,
      _ => false,
    })
    .count()
}

/// How often a limit of the run of `events` was raised, whoever raised it: the events do not say who.
pub fn raises(events: &[RunEvent]) -> usize {
  events
    .iter()
    .filter(|event| {
      matches!(
        event.kind(),
        RunEventKind::LimitRaised { .. } | RunEventKind::StepBudgetRaised { .. }
      )
    })
    .count()
}
