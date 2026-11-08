//! A new team resource is created as its first version (`docs/design/team-configuration.md`, decision 6): it is kept
//! here as published, waiting for the server, and counts as created only once the server accepts it. A server that
//! refuses it takes it back whole, so nothing shows as created; one that cannot be reached leaves it in the outbox
//! (`docs/architecture/server.md`, row 36), sent on reconnect.

use std::path::Path;

use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::ResourceKind,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, PublishState};

use super::{
  Servers, SharingError,
  project::ProjectServer,
  send_lane::{between_sends, send_turn},
  sync::RemoteResourceStore,
  team::said,
  team_edit::Refusal,
};
use crate::{
  background,
  messages::{self, Message},
};

/// How a new resource of a project stands once [`Servers::send_created`] ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Created {
  /// Created: a local project's, or one the team server accepted.
  Accepted,
  /// The team server cannot be reached now: it waits in the outbox and is sent on reconnect.
  Waiting,
}

/// Why a new team resource was not created; nothing of it is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotCreated {
  /// The team server refused it with its error `code`.
  Refused { code: String },
  /// Its body names an MCP server of this computer that is not a team resource, or (of a kind whose body is not in the
  /// team's form yet) a connection or secret of this computer, which a team body may not
  /// (`SharingError::LocalEntitiesNamed`), or it is a profile with writable folders, which are paths of this computer
  /// (`SharingError::WritableFoldersStay`); nothing was sent.
  NamesLocal(Message),
  /// It runs a profile or tool that is not the team's (`SharingError::PipelineNamesNonTeam`); nothing was sent.
  NamesNonTeam(Message),
  /// It could not be sent for a reason other than being offline.
  Failed(Message),
}

impl Servers {
  /// Sends the new resource `resource` of the project at `repo_root`, created as a version waiting for the server
  /// ([`ProjectDb::create_published`]), and settles it: accepted, waiting in the outbox while the server cannot be
  /// reached, or taken back when the server refused it or this computer found it cannot be sent. A send that failed
  /// otherwise (the server may have answered) leaves it waiting and fails with why. A local project's has nothing to
  /// send.
  pub async fn send_created(&self, repo_root: &Path, resource: ResourceId) -> Result<Created, NotCreated> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let settled = background::run(move || settle(&servers, &root, resource))
      .await
      .map_err(NotCreated::Failed)?;
    // How the team's resources stand here changed: a new one waits for the server, or was taken back.
    self.shared_changed();
    settled
  }

  /// *Cancel sending*: the new resource `resource` of the project at `repo_root`, which waits in the outbox for a team
  /// server that could not be reached, is taken back as if never created. Fails with what to tell the person when it is
  /// being sent right now, the server took it meanwhile, or the resource holds more than that version.
  pub async fn cancel_created(&self, repo_root: &Path, resource: ResourceId) -> Result<(), Message> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    background::run(move || {
      let lane = servers.send_lane(&root);
      let _between = between_sends(&lane)?;
      let mut db = ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
      db.discard_created(resource)
        .map_err(|error| messages::store_error(&error))
    })
    .await??;
    self.shared_changed();
    Ok(())
  }
}

fn settle(servers: &Servers, root: &Path, resource: ResourceId) -> Result<Created, NotCreated> {
  let failed = |error: &SharingError| NotCreated::Failed(said(error, None));
  // Nothing else sends or takes back what waits meanwhile: a cancel would delete a resource the server then numbers.
  let lane = servers.send_lane(root);
  let _sending = send_turn(&lane);
  let mut db = ProjectDb::open(root).map_err(|error| failed(&error.into()))?;
  if waiting_state(&db, resource)
    .map_err(|error| failed(&error.into()))?
    .is_none()
  {
    return Ok(Created::Accepted);
  }
  let sent = send(servers, root, resource);
  let state = waiting_state(&db, resource).map_err(|error| failed(&error.into()))?;
  if state.is_none() {
    return settled_elsewhere(servers, root, &db, resource);
  }
  let still_waiting = state == Some(PublishState::Waiting);
  let outcome = match (state, sent) {
    (Some(PublishState::Waiting), Ok(())) => return Ok(Created::Waiting),
    (Some(PublishState::Waiting), Err(error)) if error.unreached() => return Ok(Created::Waiting),
    (state, Err(error)) => refusal(state, Some(&error)),
    (state, Ok(())) => refusal(state, None),
  };
  // A send that failed for another reason than the server being out of reach may have been answered: the version stays
  // in the outbox, a retry is idempotent, and what went wrong is told. Taking it back could delete a resource the
  // server holds. What this computer found it cannot send at all (`NamesLocal`, `NamesNonTeam`) reached no server.
  if still_waiting && matches!(outcome, NotCreated::Failed(_)) {
    return Err(outcome);
  }
  // The resource that could not be taken back stays as it was, so that, not the server's answer, is what to tell.
  db.discard_created(resource)
    .map_err(|error| NotCreated::Failed(messages::store_error(&error)))?;
  Err(outcome)
}

/// The new `resource` no longer waits though this form's send did not number it: a pass of the outbox sent it first
/// ([`RemoteResourceStore::send_waiting`]), and either the server took it (created) or refused it and it was taken
/// back, which this form tells, so the hub's notice for it goes.
fn settled_elsewhere(
  servers: &Servers,
  root: &Path,
  db: &ProjectDb,
  resource: ResourceId,
) -> Result<Created, NotCreated> {
  let kept = db
    .resource(resource)
    .map_err(|error| NotCreated::Failed(messages::store_error(&error)))?;
  if kept.is_some() {
    return Ok(Created::Accepted);
  }
  Err(NotCreated::Failed(
    servers
      .dismiss_taken_back(root, resource)
      .map_or_else(messages::interrupted, |taken| taken.reason),
  ))
}

/// Why the new resource whose first version stands as `state` after `sent` failed was not created.
pub(crate) fn refusal(state: Option<PublishState>, sent: Option<&SharingError>) -> NotCreated {
  match (state, sent) {
    // This computer found the team cannot take it and recorded it as refused ([`RemoteResourceStore::send_created`]).
    (_, Some(error @ (SharingError::LocalEntitiesNamed { .. } | SharingError::WritableFoldersStay { .. }))) => {
      NotCreated::NamesLocal(said(error, None))
    }
    (_, Some(error @ SharingError::PipelineNamesNonTeam { .. })) => NotCreated::NamesNonTeam(said(error, None)),
    (Some(PublishState::Waiting) | None, Some(error)) => NotCreated::Failed(said(error, None)),
    (Some(PublishState::Refused { code }), _) => NotCreated::Refused { code },
    (Some(PublishState::Conflict { .. }), _) => NotCreated::Refused {
      code: "version_conflict".to_owned(),
    },
    (Some(PublishState::Waiting) | None, None) => NotCreated::Failed(messages::interrupted()),
  }
}

/// A new team resource the team server refused when it was sent later, after the person had left the form (a restart,
/// or the server back): it is taken back whole, like one refused at once, and this says what and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenBack {
  pub project: ProjectId,
  pub resource: ResourceId,
  pub name: String,
  pub kind: ResourceKind,
  pub reason: Message,
}

/// Sends the new `resource` of the project at `root` to its server, failing with why it was not taken; nothing for a
/// local project.
fn send(servers: &Servers, root: &Path, resource: ResourceId) -> Result<(), SharingError> {
  let Some(server) = ProjectServer::open(servers, root)? else {
    return Ok(());
  };
  RemoteResourceStore::new(ProjectDb::open(root)?, server)?.send_created(resource)
}

fn waiting_state(db: &ProjectDb, resource: ResourceId) -> Result<Option<PublishState>, orchester_store::StoreError> {
  Ok(
    db.waiting_publishes()?
      .into_iter()
      .find(|waiting| waiting.resource == resource)
      .map(|waiting| waiting.state),
  )
}

impl NotCreated {
  /// Why it was not created, in words: what the team server refused it for where that is one of the refusals a person
  /// can act on (the name is taken), else its own code, or why this computer did not send it.
  pub fn message(&self) -> Message {
    match self {
      Self::Refused { code } => match Refusal::of_code(code) {
        // A version conflict says which version, which a new resource has none of.
        Refusal::Changed | Refusal::Other(_) => {
          Message::new("appMcp.errors.teamServerRefused").with("code", code.clone())
        }
        refusal => refusal.message(),
      },
      Self::NamesLocal(reason) | Self::NamesNonTeam(reason) | Self::Failed(reason) => reason.clone(),
    }
  }
}
