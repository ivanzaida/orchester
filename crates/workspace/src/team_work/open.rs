//! Carrying on the team starts a project left open when it is opened: a start waits at its boundary while the server
//! or the runner cannot take it, and without this only the person's next Run would find it again.

use std::path::Path;

use orchester_core::secret::SecretVault;
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
  /// Some start reached its end this time: its run started, or it was settled for good (refused, or failed). The
  /// task's page tells which ([`super::last_start`]), so it is read again.
  pub settled: bool,
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
    Err(SharingError::Offline | SharingError::SignedOut) => return Ok(OpenStarts::default()),
    Err(error) => return Err(error.into()),
  };
  let dispatcher = Dispatcher::new(root.to_owned(), servers.clone(), Some(session));
  let runner = ProjectRunner::new(runners, root.to_owned(), name.to_owned());
  TeamWork::new(dispatcher, runner)
    .carry_on_open(vault)
    .await
    .map_err(Into::into)
}

impl<R: TeamRunner> TeamWork<R> {
  /// Carries on the project's open starts one by one, and says whether some reached its end.
  pub async fn carry_on_open(&self, vault: &dyn SecretVault) -> Result<OpenStarts, TeamWorkError> {
    let Some(recovered) = self.try_recover(vault).await? else {
      // A start of the project is under way, e.g. the person's Run: it carries itself on.
      return Ok(OpenStarts::default());
    };
    let mut settled = false;
    for (operation, outcome) in recovered {
      match outcome {
        Ok(StartOutcome::Started(_) | StartOutcome::Refused(..) | StartOutcome::Failed(..)) => settled = true,
        Ok(StartOutcome::Waiting(_) | StartOutcome::RunnerRefused(_) | StartOutcome::RunnerUnavailable(_)) => {}
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
    Ok(OpenStarts { settled })
  }
}
