//! Carrying on the team starts a project left open when it is opened: a start waits at its boundary while the server
//! or the runner cannot take it, and without this only the person's next Run would find it again.

use std::{collections::HashMap, path::Path};

use orchester_core::{id::OperationId, secret::SecretVault, time::Timestamp};
use orchester_store::ProjectDb;

use super::{ProjectRunner, StartOutcome, TeamRunner, TeamWork, TeamWorkError};
use crate::{
  actions::Dispatcher,
  runner::Runners,
  sharing::{Servers, SharingError, StartTrouble},
};

/// What carrying on a project's open starts did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenStarts {
  /// What the task's page tells of some start ([`super::last_start`]) changed this time, so it is read again: the
  /// start reached its end (its run started, or it was settled for good, refused or failed), or the runner's answer to
  /// it is not the one it gave the last time.
  pub changed: bool,
}

/// Why the open starts of a project could not be looked at.
#[derive(Debug, thiserror::Error)]
pub enum OpenStartsError {
  #[error(transparent)]
  Work(#[from] TeamWorkError),
  #[error(transparent)]
  Sharing(#[from] SharingError),
}

/// Carries on the starts of the project at `root`, named `name`, that are not settled yet, in a team session of the
/// project's server. A server this computer is signed out of or cannot reach leaves them open, and so does a start that
/// is under way (a Run holds the project's one start at a time) or one that could not be carried on for now; the
/// caller asks again later, also for a project that was local when it first asked and is shared since. Blocks on SQLite
/// and holds the project database across awaits: run it on a worker of the runtime.
pub async fn recover_open_starts(
  (servers, runners): (&Servers, Runners),
  (root, name): (&Path, &str),
  vault: &dyn SecretVault,
) -> Result<OpenStarts, OpenStartsError> {
  let db = ProjectDb::open(root).map_err(TeamWorkError::from)?;
  let Some(link) = db.sharing().map_err(TeamWorkError::from)? else {
    return Ok(OpenStarts::default());
  };
  if db.open_team_starts().map_err(TeamWorkError::from)?.is_empty() {
    return Ok(OpenStarts::default());
  }
  drop(db);
  let session = match servers.team_session(link.server, link.project).await {
    Ok(session) => session,
    Err(SharingError::Offline | SharingError::SignedOut) => return forget_runner_refusals(root).map_err(Into::into),
    Err(error) => return Err(error.into()),
  };
  let dispatcher = Dispatcher::new(root.to_owned(), servers.clone(), Some(session));
  let runner = ProjectRunner::new(runners, root.to_owned(), name.to_owned());
  TeamWork::new(dispatcher, runner)
    .carry_on_open(vault)
    .await
    .map_err(Into::into)
}

/// The open starts of the project at `root` wait for the team server, which cannot be reached or has this computer
/// signed out, so the runner is not asked this round: what it refused the last time no longer says where they stand,
/// and the page says that they wait. Says whether some start's kept refusal was dropped, which the page is told to read
/// again. Blocks on SQLite.
pub fn forget_runner_refusals(root: &Path) -> Result<OpenStarts, TeamWorkError> {
  let mut db = ProjectDb::open(root)?;
  let mut changed = false;
  for start in db.open_team_starts()? {
    if start.runner_refusal.is_some() {
      db.keep_team_start_runner_refusal(start.operation, None, Timestamp::now())?;
      changed = true;
    }
  }
  Ok(OpenStarts { changed })
}

impl<R: TeamRunner> TeamWork<R> {
  /// Carries on the project's open starts one by one, and says whether some reached its end.
  pub async fn carry_on_open(&self, vault: &dyn SecretVault) -> Result<OpenStarts, TeamWorkError> {
    let before = self.runner_answers()?;
    let Some(recovered) = self.try_recover(vault).await? else {
      // A start of the project is under way, e.g. the person's Run: it carries itself on.
      return Ok(OpenStarts::default());
    };
    let mut changed = false;
    for (operation, outcome) in recovered {
      match outcome {
        Ok(StartOutcome::Started(_) | StartOutcome::Refused(..) | StartOutcome::Failed(..)) => changed = true,
        Ok(StartOutcome::Waiting(_) | StartOutcome::RunnerUnavailable(_)) => {}
        // The record keeps the refusal for the task's page ([`super::LastStart::RunnerRefused`]); the log says it once
        // while the session lasts, not every round.
        Ok(StartOutcome::RunnerRefused(rejection)) => {
          if self
            .dispatcher
            .servers()
            .first_trouble(operation, StartTrouble::RunnerRefused)
          {
            tracing::warn!(?rejection, %operation, "the runner refuses an open team start, which is asked again");
          }
        }
        // Its record is kept. One that cannot be read is the same every time: it is said once, and the task's page
        // says it too ([`LastStart::Unreadable`]); it is asked again with the others, until a newer Orchester reads it.
        Err(error) if error.is_permanent() => {
          if self
            .dispatcher
            .servers()
            .first_trouble(operation, StartTrouble::Unreadable)
          {
            tracing::warn!(%error, %operation, "an open team start cannot be read, and is left");
          }
        }
        // The database was busy, or the server answered strangely: a later try may carry it on. Said once while the
        // session lasts, not every round.
        Err(error) => {
          if self
            .dispatcher
            .servers()
            .first_trouble(operation, StartTrouble::Retrying)
          {
            tracing::warn!(%error, %operation, "an open team start could not go on, and is tried again");
          }
        }
      }
    }
    let after = self.runner_answers()?;
    changed |= after
      .iter()
      .any(|(operation, answer)| before.get(operation) != Some(answer));
    Ok(OpenStarts { changed })
  }

  /// What the runner last answered to each open start: its refusal as kept, if it refused.
  fn runner_answers(&self) -> Result<HashMap<OperationId, Option<String>>, TeamWorkError> {
    Ok(
      self
        .db()?
        .open_team_starts()?
        .into_iter()
        .map(|start| (start.operation, start.runner_refusal))
        .collect(),
    )
  }
}
