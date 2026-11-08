//! Carrying on the team starts a project left open when it is opened: a start waits at its boundary while the server
//! or the runner cannot take it, and without this only the person's next Run would find it again.

use std::path::Path;

use orchester_core::secret::SecretVault;
use orchester_store::ProjectDb;

use super::{ProjectRunner, StartOutcome, TeamRunner, TeamWork, TeamWorkError};
use crate::{
  actions::Dispatcher,
  runner::Runners,
  sharing::{Servers, SharingError},
};

/// What carrying on a project's open starts left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenStarts {
  /// No start of the project is open, or the project is local.
  None,
  /// Every open start was carried on as far as it goes, or stands at a refusal of the runner that only a person's next
  /// try changes.
  Settled,
  /// A start waits for the team server or the runner, which could not be reached or taken it now, is under way, or
  /// could not be carried on: ask again later.
  Waiting,
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
/// project's server. A server this computer is signed out of or cannot reach leaves them [`OpenStarts::Waiting`], and
/// so does a start that is under way (a Run holds the project's one start at a time) or one that could not be carried
/// on. Blocks on SQLite and holds the project database across awaits: run it on a worker of the runtime.
pub async fn recover_open_starts(
  (servers, runners): (&Servers, Runners),
  (root, name): (&Path, &str),
  vault: &dyn SecretVault,
) -> Result<OpenStarts, OpenStartsError> {
  let db = ProjectDb::open(root).map_err(TeamWorkError::from)?;
  let (Some(link), false) = (
    db.sharing().map_err(TeamWorkError::from)?,
    db.open_team_starts().map_err(TeamWorkError::from)?.is_empty(),
  ) else {
    return Ok(OpenStarts::None);
  };
  drop(db);
  let session = match servers.team_session(link.server, link.project).await {
    Ok(session) => session,
    Err(SharingError::Offline | SharingError::SignedOut) => return Ok(OpenStarts::Waiting),
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
  /// Carries on the project's open starts one by one, and says what is left of them.
  pub async fn carry_on_open(&self, vault: &dyn SecretVault) -> Result<OpenStarts, TeamWorkError> {
    let Some(recovered) = self.try_recover(vault).await? else {
      // A start of the project is under way, e.g. the person's Run: it carries itself on.
      return Ok(OpenStarts::Waiting);
    };
    let mut left = OpenStarts::Settled;
    for (operation, outcome) in recovered {
      match outcome {
        Ok(StartOutcome::Waiting(_) | StartOutcome::RunnerUnavailable(_)) => left = OpenStarts::Waiting,
        Ok(_) => {}
        // Its record is kept, so a later try may carry it on (the database was busy, the server answered strangely).
        Err(error) => {
          tracing::warn!(%error, %operation, "an open team start could not go on, and is tried again");
          left = OpenStarts::Waiting;
        }
      }
    }
    Ok(left)
  }
}
