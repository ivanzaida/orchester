//! The ceilings of work led to the end as a person types them, in the form `/watching` opens for one subscription
//! and in Project settings › Orchestrator › *Leading to the end* (`Parts · Ceilings` b and c, `S01`, on the board
//! `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`): five fields of text, read
//! field by field so each one that is out of bounds says so by itself, and nothing is kept until all of them are
//! ceilings (`orchester_core::orchestrator::Ceilings`). A budget is an absolute whole number of tokens, as every
//! budget field's is (`orchester_core::tokens::parse_budget`), and empty for none.

use orchester_core::{
  orchestrator::Ceilings,
  tokens::{grouped, parse_optional_budget},
};

/// The ceilings as typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypedCeilings {
  /// How often a task may be run, its first run included.
  pub runs: String,
  /// How often a run may be returned or retried.
  pub returns: String,
  /// How often a run's limit may be raised.
  pub raises: String,
  /// By how much of the limit, in percent.
  pub percent: String,
  /// The tokens the work may spend; empty for no budget.
  pub budget: String,
}

/// A field that holds no ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrong {
  /// Not a whole number from [`Ceilings::MIN_RUNS`] to [`Ceilings::MAX_RUNS`].
  Runs,
  /// Not a whole number up to [`Ceilings::MAX_RETURNS`].
  Returns,
  /// Not a whole number up to [`Ceilings::MAX_RAISES`].
  Raises,
  /// A share below [`TypedCeilings::MIN_RAISE_PERCENT`].
  PercentBelow,
  /// No whole number, or a share above [`Ceilings::MAX_RAISE_PERCENT`].
  PercentAbove,
  /// Neither empty nor a whole number of tokens above 0.
  Budget,
}

impl TypedCeilings {
  /// The least share a limit is raised by.
  pub const MIN_RAISE_PERCENT: u8 = 1;

  /// `ceilings` as the fields show them, a budget with its digits grouped by `separator`.
  pub fn of(ceilings: Ceilings, separator: &str) -> Self {
    Self {
      runs: ceilings.runs_of_task().to_string(),
      returns: ceilings.returns_of_run().to_string(),
      raises: ceilings.raises_of_run().to_string(),
      percent: ceilings.raise_percent().to_string(),
      budget: ceilings
        .budget()
        .map(|budget| grouped(budget.get(), separator))
        .unwrap_or_default(),
    }
  }

  /// The ceilings the fields hold, a budget's digits grouped by `separator`; or every field that holds none, in
  /// the fields' order.
  pub fn read(&self, separator: &str) -> Result<Ceilings, Vec<Wrong>> {
    let runs = whole(&self.runs, Ceilings::MIN_RUNS, Ceilings::MAX_RUNS).ok_or(Wrong::Runs);
    let returns = whole(&self.returns, 0, Ceilings::MAX_RETURNS).ok_or(Wrong::Returns);
    let raises = whole(&self.raises, 0, Ceilings::MAX_RAISES).ok_or(Wrong::Raises);
    let percent = match self.percent.trim().parse::<u8>() {
      Ok(percent) if percent < Self::MIN_RAISE_PERCENT => Err(Wrong::PercentBelow),
      Ok(percent) if percent <= Ceilings::MAX_RAISE_PERCENT => Ok(percent),
      Ok(_) | Err(_) => Err(Wrong::PercentAbove),
    };
    let budget = parse_optional_budget(&self.budget, separator).map_err(|_| Wrong::Budget);
    if let (Ok(runs), Ok(returns), Ok(raises), Ok(percent), Ok(budget)) = (runs, returns, raises, percent, budget)
      && let Ok(ceilings) = Ceilings::new((runs, returns), (raises, percent), budget)
    {
      return Ok(ceilings);
    }
    Err(
      [runs.err(), returns.err(), raises.err(), percent.err(), budget.err()]
        .into_iter()
        .flatten()
        .collect(),
    )
  }
}

/// The whole number `typed` holds, from `least` to `most`.
fn whole(typed: &str, least: u8, most: u8) -> Option<u8> {
  typed
    .trim()
    .parse::<u8>()
    .ok()
    .filter(|number| (least..=most).contains(number))
}
