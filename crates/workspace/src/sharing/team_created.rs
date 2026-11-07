//! A new team resource is created as its first version (`docs/design/team-configuration.md`, decision 6): it is kept
//! here as published, waiting for the server, and counts as created only once the server accepts it. A server that
//! refuses it takes it back whole, so nothing shows as created; one that cannot be reached leaves it in the outbox
//! (`docs/architecture/server.md`, row 36), sent on reconnect.

use std::path::Path;

use orchester_core::id::ResourceId;
use orchester_store::{ProjectDb, PublishState};

use super::{Servers, SharingError, project::ProjectServer, sync::RemoteResourceStore, team::said};
use crate::{background, messages::Message};

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
  /// Its body names a connection, MCP server or secret of this computer, which a team body may not
  /// (`SharingError::LocalEntitiesNamed`); nothing was sent.
  NamesLocal(Message),
  /// It runs a profile or tool that is not the team's (`SharingError::PipelineNamesNonTeam`); nothing was sent.
  NamesNonTeam(Message),
  /// It could not be sent for a reason other than being offline.
  Failed(Message),
}

impl Servers {
  /// Sends the new resource `resource` of the project at `repo_root`, created as a version waiting for the server
  /// ([`ProjectDb::create_published`]), and settles it: accepted, waiting in the outbox while the server cannot be
  /// reached, or taken back when the server refused it or it could not be sent. A local project's has nothing to send.
  pub async fn send_created(&self, repo_root: &Path, resource: ResourceId) -> Result<Created, NotCreated> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    background::run(move || settle(&servers, &root, resource))
      .await
      .map_err(NotCreated::Failed)?
  }
}

fn settle(servers: &Servers, root: &Path, resource: ResourceId) -> Result<Created, NotCreated> {
  let failed = |error: &SharingError| NotCreated::Failed(said(error, None));
  let mut db = ProjectDb::open(root).map_err(|error| failed(&error.into()))?;
  if waiting_state(&db, resource)
    .map_err(|error| failed(&error.into()))?
    .is_none()
  {
    return Ok(Created::Accepted);
  }
  let sent = send(servers, root, resource);
  let state = waiting_state(&db, resource).map_err(|error| failed(&error.into()))?;
  let outcome = match (state, sent) {
    (None, _) => return Ok(Created::Accepted),
    (Some(PublishState::Waiting), Ok(())) => return Ok(Created::Waiting),
    (Some(PublishState::Waiting), Err(error)) if unreached(&error) => return Ok(Created::Waiting),
    // This computer found the team cannot take it and recorded it as refused ([`RemoteResourceStore::send_created`]).
    (_, Err(error @ SharingError::LocalEntitiesNamed { .. })) => NotCreated::NamesLocal(said(&error, None)),
    (_, Err(error @ SharingError::PipelineNamesNonTeam { .. })) => NotCreated::NamesNonTeam(said(&error, None)),
    (Some(PublishState::Waiting), Err(error)) => NotCreated::Failed(said(&error, None)),
    (Some(PublishState::Refused { code }), _) => NotCreated::Refused { code },
    (Some(PublishState::Conflict { .. }), _) => NotCreated::Refused {
      code: "version_conflict".to_owned(),
    },
  };
  if let Err(error) = db.discard_created(resource) {
    // The server's answer is what the caller needs; the resource that could not be taken back stays as it was.
    tracing::warn!(%resource, %error, "a refused new team resource could not be taken back");
  }
  Err(outcome)
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

/// Whether `error` says the server was not reached, so the resource waits.
fn unreached(error: &SharingError) -> bool {
  match error {
    SharingError::Offline | SharingError::SignedOut => true,
    SharingError::Client(error) => error.code().is_none(),
    _ => false,
  }
}
