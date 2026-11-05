//! A definition edit sent on its route, and what became of it. These routes keep no receipt, so when the response is
//! lost the subject is read again and compared with the edit: holding the edit, it was made; still at the revision it
//! was decided on (for a tag: without the edit), it was not, and it is sent again, which that revision or the route's
//! key makes safe; changed otherwise, it changed meanwhile and the edit is [`Refusal::Superseded`]. A revision
//! conflict reads the current value the same way, so the same intent sent again after a lost response finds its own
//! edit made rather than refused. An accepted edit stays accepted whatever this computer does next.

use orchester_api::{
  ErrorCode, SharedModule, SharedTag, SharedTask, SharingErrorDetails, TaskChanges,
  client::{AuthorityCallError, ClientError, PolicyChange},
  v2::{WorkspacePolicy, WorkspacePolicyChanges},
};
use orchester_core::id::OperationId;

use super::{DefinitionEdit, TagEdit};
use crate::{
  actions::{Accepted, ActionError, Current, Definition, Refusal, TeamOutcome},
  team_reads::TeamSession,
};

/// How many times an edit is sent while its response is lost and the subject read again shows it was not made.
const SENDS: usize = 2;

/// What the server answered to an edit it took.
enum Answer {
  Made(Definition),
  /// `PATCH …/policy`'s revision conflict, and what the policy as it is now says of the change.
  PolicyConflict(Seen),
}

/// What the subject as it is now says of the edit.
enum Seen {
  Made(Definition),
  NotMade,
  Moved(Current),
  Unread(AuthorityCallError),
}

/// Sends `edit`, the intent `operation`'s, in `session`, and learns what became of it.
pub(in crate::actions) async fn carry_out(
  session: &TeamSession,
  operation: OperationId,
  edit: &DefinitionEdit,
) -> Result<TeamOutcome, ActionError> {
  for _ in 0..SENDS {
    if !session.current() {
      return Err(ActionError::SessionChanged);
    }
    match send(session, edit).await {
      Ok(Answer::Made(definition)) => return Ok(completed(definition)),
      Ok(Answer::PolicyConflict(seen)) => return Ok(conflict(operation, Refusal::PolicyRevisionConflict, seen)),
      Err(ClientError::Api {
        status, code, details, ..
      }) => return Ok(refused(session, operation, edit, (status, code, details)).await),
      // Lost or unreadable: whether the server made it is read below.
      Err(_) => {}
    }
    match look(session, edit).await {
      Seen::Made(definition) => return Ok(completed(definition)),
      Seen::NotMade => {}
      Seen::Moved(current) => return Ok(rejected(operation, Refusal::Superseded, Some(current))),
      Seen::Unread(_) => break,
    }
  }
  Ok(TeamOutcome::AcceptanceUnknown { operation })
}

async fn send(session: &TeamSession, edit: &DefinitionEdit) -> Result<Answer, ClientError> {
  let (client, token) = session.client();
  let project = session.project();
  let made = match edit {
    DefinitionEdit::Task { task, patch } => {
      Definition::Task(Box::new(client.update_task(token, project, *task, patch).await?))
    }
    DefinitionEdit::CreateModule(create) => {
      Definition::Module(Box::new(client.create_module(token, project, create).await?))
    }
    DefinitionEdit::Module { module, patch } => {
      Definition::Module(Box::new(client.update_module(token, project, *module, patch).await?))
    }
    DefinitionEdit::Tag(TagEdit::Create(name)) => Definition::Tag(client.create_tag(token, project, name).await?),
    DefinitionEdit::Tag(TagEdit::Rename { tag, name }) => {
      Definition::Tag(client.rename_tag(token, project, *tag, name).await?)
    }
    DefinitionEdit::Tag(TagEdit::Merge { tag, into }) => tags(client.merge_tag(token, project, *tag, into).await?),
    DefinitionEdit::Tag(TagEdit::Delete(tag)) => tags(client.delete_tag(token, project, *tag).await?),
    DefinitionEdit::Policy(changes) => {
      return Ok(match session.authority().change_policy(changes).await? {
        PolicyChange::Changed(response) => Answer::Made(Definition::Policy(response.policy)),
        PolicyChange::Conflict(current) => Answer::PolicyConflict(policy_seen(*changes, current)),
      });
    }
  };
  Ok(Answer::Made(made))
}

fn tags(changed: orchester_api::TagsChanged) -> Definition {
  Definition::Tags {
    tags: changed.tags,
    tasks: Some(changed.tasks),
  }
}

/// The route's refusal. A revision conflict comes with the subject as it is now, from the refusal or read again; one
/// whose subject holds the edit is that edit, made by an earlier send.
async fn refused(
  session: &TeamSession,
  operation: OperationId,
  edit: &DefinitionEdit,
  (status, code, details): (u16, ErrorCode, Option<Box<SharingErrorDetails>>),
) -> TeamOutcome {
  let refusal = Refusal::Route { status, code };
  if code != ErrorCode::RevisionConflict {
    return TeamOutcome::Rejected {
      operation,
      refusal,
      retryable: retryable(code),
      current: None,
    };
  }
  let seen = match (edit, details.map(|details| *details)) {
    (DefinitionEdit::Task { patch, .. }, Some(SharingErrorDetails::Revision { current })) => {
      task_seen(&patch.changes, patch.revision, *current)
    }
    (DefinitionEdit::Module { patch, .. }, Some(SharingErrorDetails::ModuleRevision { current })) => {
      module_seen(patch, *current)
    }
    _ => look(session, edit).await,
  };
  conflict(operation, refusal, seen)
}

/// A conflict's outcome by what the subject as it is now says of the edit.
fn conflict(operation: OperationId, refusal: Refusal, seen: Seen) -> TeamOutcome {
  match seen {
    Seen::Made(definition) => completed(definition),
    Seen::NotMade => rejected(operation, refusal, None),
    Seen::Moved(current) => rejected(operation, refusal, Some(current)),
    Seen::Unread(error) => rejected(operation, refusal, Some(Current::NotRead(error))),
  }
}

/// Refusals that change nothing and pass: signed out, rate limited, timed out or failed on the server.
const fn retryable(code: ErrorCode) -> bool {
  matches!(
    code,
    ErrorCode::Unauthorized | ErrorCode::RateLimited | ErrorCode::RequestTimeout | ErrorCode::Internal
  )
}

const fn completed(definition: Definition) -> TeamOutcome {
  TeamOutcome::Completed(Accepted::Definition(definition))
}

const fn rejected(operation: OperationId, refusal: Refusal, current: Option<Current>) -> TeamOutcome {
  TeamOutcome::Rejected {
    operation,
    refusal,
    retryable: false,
    current,
  }
}

/// The subject of `edit` read again. A module's edit is keyed: sending it again answers the first, so it is not read.
async fn look(session: &TeamSession, edit: &DefinitionEdit) -> Seen {
  let (client, token) = session.client();
  let project = session.project();
  match edit {
    DefinitionEdit::Task { task, patch } => match client.task(token, project, *task).await {
      Ok(current) => task_seen(&patch.changes, patch.revision, current),
      Err(error) => Seen::Unread(error.into()),
    },
    DefinitionEdit::CreateModule(_) | DefinitionEdit::Module { .. } => Seen::NotMade,
    DefinitionEdit::Tag(edit) => match client.tags(token, project).await {
      Ok(current) => tags_seen(edit, current),
      Err(error) => Seen::Unread(error.into()),
    },
    DefinitionEdit::Policy(changes) => match session.authority().policy().await {
      Ok(current) => policy_seen(*changes, current.policy),
      Err(error) => Seen::Unread(error),
    },
  }
}

fn task_seen(changes: &TaskChanges, read: orchester_core::sharing::TaskRevision, task: SharedTask) -> Seen {
  if holds(changes, &task) {
    return Seen::Made(Definition::Task(Box::new(task)));
  }
  if task.revision == read {
    return Seen::NotMade;
  }
  Seen::Moved(Current::Task(Box::new(task)))
}

/// Whether `task` has every field `changes` sets as they set it.
fn holds(changes: &TaskChanges, task: &SharedTask) -> bool {
  let content = &task.content;
  same(changes.title.as_ref(), &content.title)
    && same(changes.brief.as_ref(), &content.brief)
    && same(changes.criteria.as_ref(), &content.criteria)
    && same(changes.pipeline.as_ref(), &content.pipeline)
    && same(changes.priority.as_ref(), &content.priority)
    && same(changes.tags.as_ref(), &content.tags)
    && same(changes.blocking.as_ref(), &content.blocking)
    && same(changes.blocked_reason.as_ref(), &content.blocked_reason)
    && same(changes.module.as_ref(), &content.module)
}

fn same<T: PartialEq>(change: Option<&T>, value: &T) -> bool {
  change.is_none_or(|change| change == value)
}

fn module_seen(patch: &orchester_api::ModulePatch, module: SharedModule) -> Seen {
  if module.fields == patch.module {
    return Seen::Made(Definition::Module(Box::new(module)));
  }
  if module.revision == patch.revision {
    return Seen::NotMade;
  }
  Seen::Moved(Current::Module(Box::new(module)))
}

/// The policy holds the change once its revision moved past the one read with the value asked for.
fn policy_seen(changes: WorkspacePolicyChanges, policy: WorkspacePolicy) -> Seen {
  if policy.revision.get() > changes.revision.get() && policy.automatic_merge == changes.automatic_merge {
    return Seen::Made(Definition::Policy(policy));
  }
  if policy.revision == changes.revision {
    return Seen::NotMade;
  }
  Seen::Moved(Current::Policy(policy))
}

fn tags_seen(edit: &TagEdit, tags: Vec<SharedTag>) -> Seen {
  let find = |id| tags.iter().find(|tag| tag.id == id).cloned();
  match edit {
    TagEdit::Create(name) => match tags.iter().find(|tag| tag.name == name.name) {
      Some(tag) => Seen::Made(Definition::Tag(tag.clone())),
      None => Seen::NotMade,
    },
    TagEdit::Rename { tag, name } => match find(*tag) {
      Some(tag) if tag.name == name.name => Seen::Made(Definition::Tag(tag)),
      Some(_) => Seen::NotMade,
      None => Seen::Moved(Current::Tags(tags)),
    },
    TagEdit::Merge { tag, .. } | TagEdit::Delete(tag) => match find(*tag) {
      Some(_) => Seen::NotMade,
      None => Seen::Made(Definition::Tags { tags, tasks: None }),
    },
  }
}
