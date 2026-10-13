//! Desktop signal values for session readings and the project's CLI connections.
use orchester_core::connection::{AgentConnection, plan_limits::PlanReadings};

use crate::messages::Message;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Readings(pub PlanReadings);

/// The project's agent connections, as the plan limits item reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Connections {
  /// Not read yet: the item is not shown until it is known whether the project has agents.
  #[default]
  Loading,
  Ready(Vec<AgentConnection>),
  /// They could not be read; the item says so instead of disappearing.
  Failed(Message),
}

/// One stored report of an account's plan limits, with its account resolved: what the Usage page's plan limits chart
/// draws one legend entry per account and window from (`docs/design/agent-accounts.md`, `XPjSc` › `KzIJ1`). A report
/// from before accounts names none; it was its CLI's Default's.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountReading {
  pub account: orchester_core::id::AgentAccountId,
  pub snapshot: orchester_core::usage::limits::LimitSnapshot,
}

/// The plan limits reports the project at `repo_root` stored from `since` on, oldest first, each with its account.
/// A report of a CLI that has no accounts (a native connection) is left out.
pub fn account_readings_in(
  repo_root: &std::path::Path,
  since: orchester_core::time::Timestamp,
) -> Result<Vec<AccountReading>, orchester_store::StoreError> {
  let db = orchester_store::ProjectDb::open(repo_root)?;
  Ok(
    db.limit_snapshots(since)?
      .into_iter()
      .filter_map(|snapshot| {
        let account = snapshot.account.or_else(|| {
          orchester_core::account::AccountCli::of(snapshot.provider)
            .map(orchester_core::account::AccountCli::default_account)
        })?;
        Some(AccountReading { account, snapshot })
      })
      .collect(),
  )
}
