//! Whether the project's reviewer can run (ORC-611), as Project settings › Runs and a request's card that came to the
//! person say it: the account its connection runs on — its sign-in and newest plan limits reading, from the
//! application database and every project — and how its own last review and the estimator's last request ended.

use std::path::Path;

use orchester_core::{
  connection::ConnectionKind,
  run::review::{ReviewEnd, ReviewFailure, ReviewerAccount, ReviewerHealth, resolve_reviewer},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;

use crate::{accounts::AccountsError, workspace::Workspace};

impl Workspace {
  /// The reviewer of the project at `repo_root`, as far as Orchester knows whether it can run; the default (nothing
  /// known) for a project without one, and for a decision model, which has no account and whose failures its own
  /// line on the approval card says.
  pub fn reviewer_health(&self, repo_root: &Path) -> Result<ReviewerHealth, AccountsError> {
    let db = ProjectDb::open(repo_root)?;
    let (settings, connections) = (db.auto_mode_settings()?, db.connections()?);
    let Ok(reviewer) = resolve_reviewer(&settings, &connections, &[]) else {
      return Ok(ReviewerHealth::default());
    };
    let (Some(connection), Some(provider)) = (reviewer.connection.language(), reviewer.kind.provider()) else {
      return Ok(ReviewerHealth::default());
    };
    let account = match *connection.kind() {
      ConnectionKind::Agent(provider) => connection.account().id(provider),
      ConnectionKind::GitHub { .. } => None,
    };
    let account = match account {
      Some(id) => ReviewerAccount {
        state: self.agent_account(id)?.map(|account| account.state()),
        reading: self.last_plan_reading(id)?.map(|(reading, _)| reading),
      },
      None => ReviewerAccount::default(),
    };
    Ok(ReviewerHealth {
      account,
      last_review: failure(db.last_review_end(connection.id())?),
      last_estimate: db.last_estimate_failure((provider, &reviewer.model))?,
    })
  }
}

/// Why a review that ended `end` gave no verdict; `None` for one that gave it.
const fn failure(end: Option<ReviewEnd>) -> Option<ReviewFailure> {
  match end {
    Some(ReviewEnd::Failed(failure)) => Some(failure),
    Some(ReviewEnd::Approved | ReviewEnd::Asked) | None => None,
  }
}
