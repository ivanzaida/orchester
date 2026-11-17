//! A team task's organization as `create_task` and `update_task` name it (OA-35), resolved on its team server as a
//! local task's is in its project (`task_organization`): a blocking task by its key or its server id among the team's
//! tasks, with the gate the form offers; a tag by its name among the team's tags, matched ignoring case, and created
//! there when the team has none of that name (an Admin's change, as the task's own). Every name is read before any tag
//! is made.

use orchester_api::{Blocking, SharedTag};
use orchester_core::{
  id::{TagId, TaskId},
  name::Name,
  task::organization::{BlockingTask, WorkflowStatus},
};
use orchester_store::StoreError;

use super::{Team, on_worker, outcome::accepted, read_refused, tasks::find};
use crate::{
  actions::{Accepted, Action, ActionOutcome, Definition, Dispatcher, Intent, TagAction},
  app_mcp::{
    Services,
    error::ToolError,
    tools::task_organization::{Blocker, Gate},
  },
  messages::{self, Message},
  team_reads::{TeamProject, TeamReadError},
};

/// The team tasks `named` names as blocking tasks, each by its key or server id, with its gate (`merge` when left out).
pub(super) async fn blockers(project: &TeamProject, named: &[Blocker]) -> Result<Vec<BlockingTask>, ToolError> {
  let mut blocking = Vec::new();
  for blocker in named {
    let task = find(project, &blocker.task).await?;
    blocking.push(BlockingTask {
      task: task.task.id,
      gate: blocker.gate.unwrap_or(Gate::Merge).into(),
    });
  }
  Ok(blocking)
}

/// The team's tags `names` name, in their order; those the team lacks are made on its server first, as the task
/// form's picker makes them. A name that is no name is refused before anything is sent.
pub(super) async fn tags(services: &Services, team: &Team, names: &[String]) -> Result<Vec<TagId>, ToolError> {
  let names = names
    .iter()
    .map(|named| Name::new(named))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| ToolError::refused("invalid_tag", vec![messages::name_error(&error)]))?;
  if names.is_empty() {
    return Ok(Vec::new());
  }
  let session = team.session(services).await?;
  let (client, token) = session.client();
  let known = client
    .tags(token, session.project())
    .await
    .map_err(|error| read_refused(&TeamReadError::from(error)))?;
  let lacking = names.iter().any(|name| named(&known, name).is_none());
  if !lacking {
    return Ok(
      names
        .iter()
        .filter_map(|name| named(&known, name))
        .map(|tag| tag.id)
        .collect(),
    );
  }
  let dispatcher = team.dispatcher(services).await?;
  on_worker(move || async move {
    let mut known = known;
    let mut ids = Vec::new();
    for name in names {
      let tag = match named(&known, &name) {
        Some(tag) => tag,
        None => {
          let made = made(&dispatcher, name).await?;
          known.push(made.clone());
          made
        }
      };
      ids.push(tag.id);
    }
    Ok(ids)
  })
  .await
}

/// The tag of `known` named `name`, ignoring case.
fn named(known: &[SharedTag], name: &Name) -> Option<SharedTag> {
  let lowered = name.as_str().to_lowercase();
  known.iter().find(|tag| tag.name.to_lowercase() == lowered).cloned()
}

/// The tag `name` made on the team server through `dispatcher`.
async fn made(dispatcher: &Dispatcher, name: Name) -> Result<SharedTag, ToolError> {
  let outcome = dispatcher
    .dispatch(&Intent::new(Action::Tag(TagAction::Create { name })))
    .await
    .map_err(super::outcome::action_refused)?;
  match outcome {
    ActionOutcome::Team(outcome) => match accepted(outcome)? {
      Accepted::Definition(Definition::Tag(tag)) => Ok(tag),
      _ => Err(not_a_tag()),
    },
    ActionOutcome::Local(_) => Err(not_a_tag()),
  }
}

/// The team server answered a tag's creation with something other than a tag.
fn not_a_tag() -> ToolError {
  ToolError::refused(
    "team_action_failed",
    vec![Message::new("appMcp.errors.teamActionFailed")],
  )
}

/// The tasks of `blocking` the team server does not hold Done, read there now: a team task waits for them as a local
/// task waits for its blocking tasks' work to be merged (a team task is Done once its work is merged, OA-30). Empty
/// when none holds it back.
pub(super) async fn unfinished(
  project: &TeamProject,
  blocking: &[Blocking],
) -> Result<Vec<(TaskId, String)>, ToolError> {
  let mut left = Vec::new();
  for link in blocking {
    let task = find(project, &link.task.to_string()).await?;
    if task.task.content.workflow != WorkflowStatus::Done {
      left.push((link.task, task.task.key));
    }
  }
  Ok(left)
}

/// The refusal of a run of a team task held back by the unfinished tasks `left`, as a local task's start is refused.
pub(super) fn blocked(left: &[(TaskId, String)]) -> ToolError {
  let ids = left.iter().map(|(task, _)| *task).collect();
  StoreError::Organization(orchester_core::task::organization::OrganizationError::Blocked(ids)).into()
}
