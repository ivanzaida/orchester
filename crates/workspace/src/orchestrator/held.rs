//! What holds the turns Orchester starts by itself in a project (`orchester_core::orchestrator::hold`,
//! `docs/plan/orchestrator-autonomy/OA-03-budget-and-plan-limit-hold.md`): while the project's daily budget is used,
//! or the account the Orchestrator runs on is at its plan limit, the follower starts none ([`super::waits`]) and the
//! events wait; their turns start at the reset, when the app's follower wakes ([`super::turns::next_due_in`]).

use std::path::Path;

use orchester_core::{
  connection::AgentConnection,
  orchestrator::{Held, PlanHold},
  store::{ActivityStore, ConnectionStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

/// What holds the turns Orchester starts by itself in the project at `root` at `now`; `None` while nothing does.
/// Blocks on SQLite.
pub fn held_in(root: &Path, now: Timestamp) -> Result<Option<Held>, Message> {
  ProjectDb::open_read_only(root)
    .and_then(|db| held(&db, now))
    .map_err(|error| messages::store_error(&error))
}

/// What holds the turns Orchester starts by itself at `now`, as `db` has it: today's budget, counted as the runner
/// counts it for a run, else the plan limit kept of the account the Orchestrator runs on now.
pub(super) fn held(db: &ProjectDb, now: Timestamp) -> Result<Option<Held>, StoreError> {
  let used_today = db.tokens_used_since(now.local_day_start())?;
  let budget_used = db.project_limits()?.daily_budget_used(used_today);
  Ok(Held::at(now, budget_used, plan_hold(db)?))
}

/// The plan limit kept of the account the Orchestrator's published connection runs on; `None` while it has none, or
/// nothing is published to run on.
fn plan_hold(db: &ProjectDb) -> Result<Option<PlanHold>, StoreError> {
  let Some(settings) = db.orchestrator_settings()? else {
    return Ok(None);
  };
  let Some(connection) = db.connection(settings.connection())? else {
    return Ok(None);
  };
  // A connection the Orchestrator cannot run on starts no turn: nothing of its account holds one.
  let Ok(connection) = AgentConnection::try_from(connection) else {
    return Ok(None);
  };
  match connection.account().id(connection.provider()) {
    Some(account) => db.orchestrator_hold(account),
    None => Ok(None),
  }
}
