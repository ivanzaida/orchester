//! A team pipeline's save waits for the team server (D-042, D-043): its steps are sent as the next version, a new one
//! as its first, and a rename as the resource's PATCH on the revision this computer has, at once, and kept here only
//! once the server took them. A refusal, or a server that cannot be reached, stores nothing and deletes nothing: the
//! person's edit stays theirs, with why. Nothing of a pipeline waits in the outbox. Only an admin changes it (row 72);
//! this computer refuses one it last read as no admin before anything is sent. Blocking: each runs on a worker of the
//! application's runtime, whose requests it waits for.

use std::path::Path;

use orchester_api::{
  ErrorCode, ResourceCreate, ResourcePatch, SharedResourceKind, SharingErrorDetails, VersionPublish,
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  resource::{Pipeline, Slug, VersionNumber},
};
use orchester_store::{ProjectDb, ResourceSharing, StoreError};
use tokio::runtime::Handle;

use super::ProjectServer;
use crate::{
  messages::{self, Message},
  sharing::{
    Servers, SharingError,
    sync::{ConfigServer, keep_pipeline_version, keep_resource, pipeline_content},
    team::said,
    team_edit::{Local, Refusal, recorded},
  },
};

/// Why a team pipeline's change was not taken; nothing of it was stored here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotTaken {
  /// The pipeline changed on the team server since this computer read it: someone published `latest` from the same
  /// version (a version conflict), or, for a rename, its revision there moved on (`None`).
  Changed { latest: Option<VersionNumber> },
  /// The team server refused it, or this computer, which knows the team cannot take it, did not send it.
  Refused(Refusal),
  /// The team server could not be reached, or nobody is signed in to it: nothing was sent.
  Unreached(Message),
  /// It failed otherwise. The server may have taken it: then it shows here once the team's changes are read.
  Failed(Message),
}

impl NotTaken {
  /// Why, in words: what the page says where a save's refusal shows, and what a tool answers.
  pub fn message(&self) -> Message {
    match self {
      Self::Changed { latest: Some(latest) } => {
        Message::new("pipelines.problems.teamChanged").with("version", latest.get().to_string())
      }
      Self::Changed { latest: None } => Message::new("pipelines.problems.teamRenameChanged"),
      Self::Refused(Refusal::NotAdmin | Refusal::NotAdminHere) => Message::new("sharing.errors.notAdmin"),
      Self::Refused(refusal) => refusal.message(),
      Self::Unreached(reason) | Self::Failed(reason) => reason.clone(),
    }
  }

  /// The code a tool refuses with.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::Changed { .. } => "version_changed",
      Self::Refused(Refusal::NotAdmin | Refusal::NotAdminHere) => "not_admin",
      Self::Refused(Refusal::Local(Local::NamesNonTeam)) => "pipeline_names_non_team",
      Self::Refused(Refusal::Local(_)) => "local_entities_named",
      Self::Refused(_) => "team_server_refused",
      Self::Unreached(_) => "team_server_not_reached",
      Self::Failed(_) => "team_server_failed",
    }
  }
}

impl Servers {
  /// Sends `body` as the next version of the team pipeline `resource` of the project at `repo_root`, edited from its
  /// version `based_on`, and keeps it once the server numbered it; that number.
  pub fn publish_team_pipeline(
    &self,
    repo_root: &Path,
    resource: ResourceId,
    based_on: VersionNumber,
    body: &Pipeline,
  ) -> Result<VersionNumber, NotTaken> {
    let (server, mut db) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let taken = |error: SharingError| not_taken(error, Some(&address));
    let content = pipeline_content(&db, resource, body).map_err(taken)?;
    let publish = VersionPublish {
      idempotency_key: IdempotencyKey::generate(),
      based_on: Some(based_on),
      body: content.body,
      references: content.references,
    };
    let version = runtime()?
      .block_on(server.publish_version(resource, publish))
      .map_err(taken)?;
    keep_pipeline_version(&mut db, None, &version, body).map_err(|error| failed(&error))?;
    self.shared_changed();
    Ok(version.number)
  }

  /// Creates the team pipeline `name`, under `slug`, with `body` as its first version in the project at `repo_root`,
  /// and keeps it once the server made it; its id.
  pub fn create_team_pipeline(
    &self,
    repo_root: &Path,
    (slug, name): (Slug, Name),
    body: &Pipeline,
  ) -> Result<ResourceId, NotTaken> {
    let (server, mut db) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let taken = |error: SharingError| not_taken(error, Some(&address));
    let id = ResourceId::generate();
    let create = ResourceCreate {
      idempotency_key: IdempotencyKey::generate(),
      id,
      kind: SharedResourceKind::Pipeline,
      slug: slug.as_str().to_owned(),
      name: name.as_str().to_owned(),
      version: pipeline_content(&db, id, body).map_err(taken)?,
    };
    let created = runtime()?.block_on(server.create_resource(create)).map_err(taken)?;
    keep_pipeline_version(&mut db, Some(&created.resource), &created.version, body).map_err(|error| failed(&error))?;
    self.shared_changed();
    Ok(id)
  }

  /// Renames the team pipeline `resource` of the project at `repo_root` to `name` on the revision this computer has of
  /// it, and keeps the name once the server took it.
  pub fn rename_team_pipeline(&self, repo_root: &Path, resource: ResourceId, name: &Name) -> Result<(), NotTaken> {
    let (server, mut db) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let Some(ResourceSharing::Shared(revision)) =
      db.resource_sharing(resource).map_err(|error| failed(&error.into()))?
    else {
      return Err(failed(&SharingError::Store(StoreError::ResourceNotFound(resource))));
    };
    let patch = ResourcePatch {
      idempotency_key: IdempotencyKey::generate(),
      revision,
      name: Some(name.as_str().to_owned()),
      archived: None,
    };
    let renamed =
      runtime()?
        .block_on(server.call(|client, token, project| async move {
          client.update_resource(&token, project, resource, &patch).await
        }))
        .map_err(|error| not_taken(error, Some(&address)))?;
    keep_resource(&mut db, &renamed).map_err(|error| failed(&error))?;
    self.shared_changed();
    Ok(())
  }
}

/// The server of the project at `root`, which must take changes now, and its database, where the person is an admin.
fn open(servers: &Servers, root: &Path) -> Result<(ProjectServer, ProjectDb), NotTaken> {
  let server = ProjectServer::open(servers, root)
    .map_err(|error| not_taken(error, None))?
    .ok_or_else(|| failed(&SharingError::Store(StoreError::NotShared)))?;
  server
    .writable()
    .map_err(|error| not_taken(error, Some(&server.entry().address)))?;
  let db = ProjectDb::open(root).map_err(|error| failed(&error.into()))?;
  if !db.may_change_configuration().map_err(|error| failed(&error.into()))? {
    return Err(NotTaken::Refused(Refusal::NotAdminHere));
  }
  Ok((server, db))
}

fn runtime() -> Result<Handle, NotTaken> {
  Handle::try_current().map_err(|_| NotTaken::Failed(messages::interrupted()))
}

fn failed(error: &SharingError) -> NotTaken {
  NotTaken::Failed(said(error, None))
}

/// What `error`, from the server at `address` when it was opened, says of the change.
fn not_taken(error: SharingError, address: Option<&orchester_api::client::ServerAddress>) -> NotTaken {
  if let SharingError::Client(client) = &error
    && let Some(code) = client.code()
  {
    return match (code, client.details()) {
      (ErrorCode::VersionConflict, Some(SharingErrorDetails::Version { latest })) => NotTaken::Changed {
        latest: Some(latest.number),
      },
      (ErrorCode::RevisionConflict, _) => NotTaken::Changed { latest: None },
      (code, details) => NotTaken::Refused(Refusal::of_code(&recorded(code, details))),
    };
  }
  match error {
    SharingError::NotAdmin => NotTaken::Refused(Refusal::NotAdminHere),
    SharingError::PipelineNamesNonTeam { .. } => NotTaken::Refused(Refusal::Local(Local::NamesNonTeam)),
    SharingError::LocalEntitiesNamed { .. } => NotTaken::Refused(Refusal::Local(Local::Entities)),
    SharingError::Offline => NotTaken::Unreached(Message::new("pipelines.problems.teamUnreached")),
    SharingError::Client(_) if error.unreached() => {
      NotTaken::Unreached(Message::new("pipelines.problems.teamUnreached"))
    }
    error if error.unreached() => NotTaken::Unreached(said(&error, address)),
    error => NotTaken::Failed(said(&error, address)),
  }
}
