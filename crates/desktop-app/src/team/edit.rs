//! A team task's edit from Edit task (team authority P6d, ORC-734): its details, its organization and its module go to
//! the server through the dispatcher, each on its own definition route and on the revision the one before it left, as
//! the domain MCP tools send them (`orchester_workspace::app_mcp`). The first refusal stops the rest; what was taken
//! before it stays taken, and the edit says so with the task as the server then holds it. The pipeline is one of the
//! team's, sent by its id (`docs/architecture/server.md` row 80); a limits change waits for a team-authority decision
//! (D6) and is refused before it is sent.

use orchester_api::{SharedTask, v2::AuthorityErrorCode};
use orchester_core::{
  id::{ModuleId, TaskId},
  sharing::TaskRevision,
  task::{TaskDetails, organization::TaskOrganization},
};
use orchester_workspace::{
  actions::{Accepted, Action, ActionError, ActionOutcome, Definition, Intent, TaskAction, TeamOutcome},
  background,
};

use super::{
  TeamSource,
  act::{Acted, Acting, acted},
  invalidation::Stale,
};

/// One change of a team task, as read and as edited.
#[derive(Clone, Debug)]
pub enum TaskChange {
  Details(TaskDetails, TaskDetails),
  Organization(TaskOrganization, TaskOrganization),
  Module(Option<ModuleId>),
}

impl TaskChange {
  /// The action that makes this change of `task` read at `read`.
  fn action(&self, task: TaskId, read: TaskRevision) -> TaskAction {
    match self {
      Self::Details(base, edited) => TaskAction::Edit {
        task,
        read,
        base: base.clone(),
        edited: edited.clone(),
      },
      Self::Organization(base, edited) => TaskAction::Organize {
        task,
        read,
        base: base.clone(),
        edited: edited.clone(),
      },
      Self::Module(module) => TaskAction::Move {
        task,
        read,
        module: *module,
      },
    }
  }
}

/// What a team task's edit came to.
#[derive(Debug)]
pub struct Edited {
  /// The task as the server holds it after the changes it took; `None` when it took none.
  pub saved: Option<Box<SharedTask>>,
  /// Why the server did not take the next change; `None` once it took every one.
  pub stopped: Option<Acted>,
}

impl Edited {
  const fn stopped(acted: Acted) -> Self {
    Self {
      saved: None,
      stopped: Some(acted),
    }
  }
}

impl Acting {
  /// Sends `changes` of the team task `task`, read at `read`, to `source`'s server in order; once the server answered,
  /// the session's reads of the project are read again.
  pub async fn edit_task(
    &self,
    source: &TeamSource,
    (task, read): (TaskId, TaskRevision),
    changes: Vec<TaskChange>,
  ) -> Edited {
    let (acting, source) = (self.clone(), source.clone());
    let runtime = self.runtime.clone();
    let edited = background::run(move || {
      runtime.block_on(async move {
        let Some(dispatcher) = acting.dispatcher(&source).await else {
          return Edited::stopped(Acted::SignedOut);
        };
        let mut edited = Edited {
          saved: None,
          stopped: None,
        };
        let mut read = read;
        for change in changes {
          let intent = Intent::new(Action::Task(change.action(task, read)));
          match taken(dispatcher.dispatch(&intent).await) {
            Ok(Some(shared)) => {
              read = shared.revision;
              edited.saved = Some(shared);
            }
            Ok(None) => {}
            Err(acted) => {
              edited.stopped = Some(acted);
              break;
            }
          }
        }
        edited
      })
    })
    .await
    .unwrap_or_else(|_| Edited::stopped(Acted::Failed));
    if !matches!(edited.stopped, Some(Acted::SignedOut | Acted::Failed)) {
      Stale::EVERY.apply(&self.team.client());
    }
    edited
  }
}

/// The task as the server holds it once it took a change; `None` for a change of nothing; else why it did not take it.
fn taken(outcome: Result<ActionOutcome, ActionError>) -> Result<Option<Box<SharedTask>>, Acted> {
  match outcome {
    Ok(ActionOutcome::Team(TeamOutcome::Completed(Accepted::Definition(Definition::Task(shared))))) => Ok(Some(shared)),
    Err(ActionError::NothingToChange) => Ok(None),
    Err(ActionError::DecisionRequired(_)) => Err(Acted::Refused(AuthorityErrorCode::DecisionRequired)),
    outcome => match acted(outcome).0 {
      // The server answered a task's change with something other than the task: nothing to go on.
      Acted::Done => Err(Acted::Failed),
      acted => Err(acted),
    },
  }
}
