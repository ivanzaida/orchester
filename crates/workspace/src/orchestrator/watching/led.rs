//! What the `/watching` answer says of a subscription that leads its target to the end (`L02` `HpcQJ` and `Parts ·
//! Ceilings` a on the board `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`):
//! its ceilings with what its work spent of its budget, and, for a task that was run more than once, which of the runs
//! its ceiling allows it is on (*run again 2 of 3*). Both are read from the project's database as the ceilings
//! themselves count them (`crate::orchestrator::ceilings`), never from an agent.

use std::collections::HashMap;

use orchester_core::{
  id::TaskId,
  orchestrator::{Ceilings, Subscription, Subscriptions, WatchTarget},
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{messages::Message, orchestrator::ceilings::spent_among, tasks::recorded};

/// A subscription that leads its target to the end, as its row shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedToEnd {
  pub ceilings: Ceilings,
  /// What its work spent since it leads, while it has a budget; `None` for one without, whose spending nothing
  /// bounds and nothing reads.
  pub spent: Option<TokensUsed>,
}

/// What the watch reads hold of the work led to the end; empty while no subscription leads to the end.
#[derive(Clone, Debug, Default)]
pub(super) struct LedReads {
  /// What the work of each leading subscription with a budget spent, by its target.
  spent: HashMap<WatchTarget, TokensUsed>,
  /// How many runs each of the project's tasks had.
  runs: HashMap<TaskId, usize>,
}

impl LedReads {
  /// Reads them for `subscriptions` from `db`. A project where nothing is led to the end costs no read.
  pub(super) fn read(db: &ProjectDb, subscriptions: &Subscriptions) -> Result<Self, StoreError> {
    let leads = |subscription: &&Subscription| subscription.mode().ceilings().is_some();
    if !subscriptions.as_slice().iter().any(|subscription| leads(&subscription)) {
      return Ok(Self::default());
    }
    let tasks = recorded::all_with_runs(db)?;
    let mut spent = HashMap::new();
    for subscription in subscriptions.as_slice().iter().filter(leads) {
      let budget = subscription.mode().ceilings().and_then(Ceilings::budget);
      if budget.is_some() {
        let used = spent_among(db, &tasks, (subscriptions, subscription))?;
        spent.insert(subscription.target(), used);
      }
    }
    let runs = tasks.iter().map(|(task, runs)| (task.id(), runs.len())).collect();
    Ok(Self { spent, runs })
  }

  /// `subscription` as its row shows it; `None` for one that only follows.
  pub(super) fn of(&self, subscription: &Subscription) -> Option<LedToEnd> {
    let ceilings = subscription.mode().ceilings()?;
    Some(LedToEnd {
      ceilings,
      spent: self.spent.get(&subscription.target()).copied(),
    })
  }

  /// *run again 2 of 3* for task `task`, which `ceilings` bound: the run it is on of those a task may have, once it
  /// was run more than once.
  pub(super) fn again(&self, task: TaskId, ceilings: Ceilings) -> Option<Message> {
    let runs = self.runs.get(&task).copied().filter(|runs| *runs > 1)?;
    Some(
      Message::new("orchestrator.watch.task.again")
        .with("run", runs.to_string())
        .with("most", ceilings.runs_of_task().to_string()),
    )
  }
}
