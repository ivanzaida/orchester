//! A team pipeline's save waits for the team server (D-042, D-043): its steps are sent as the next version, a new one
//! as its first, and a rename as the resource's PATCH on the revision memory has, at once. The team's configuration in
//! memory takes what the server answered; nothing of it is written to the project's database
//! (`docs/design/team-configuration-live.md`). A refusal, or a server that cannot be reached, changes nothing: the
//! person's edit stays theirs, with why. Only an admin changes it (row 72);
//! this computer refuses one it last read as no admin before anything is sent. Blocking: each runs on a worker of the
//! application's runtime, whose requests it waits for.

use std::path::Path;

use orchester_api::{
  ErrorCode, ResourceCreate, ResourcePatch, SharedResourceKind, SharingErrorDetails, VersionPublish,
  client::ClientError,
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  resource::{Pipeline, Slug, VersionNumber},
};
use orchester_store::{ProjectDb, StoreError};
use tokio::runtime::Handle;

use super::ProjectServer;
use crate::{
  messages::{self, Message},
  sharing::{
    Servers, SharingError,
    sync::{ConfigServer, pipeline_content, pipeline_executors},
    team::said,
    team_edit::{Local, Refusal, recorded},
  },
  team_config::{Changed, TeamConfig, TeamResource},
};

/// Why a team pipeline's change was not taken; nothing of it was stored here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotTaken {
  /// The pipeline changed on the team server since this computer read it: someone published `latest` from the same
  /// version (a version conflict), or, for a rename, its revision there moved on (`None`).
  Changed { latest: Option<VersionNumber> },
  /// The team server refused it, or this computer, which knows the team cannot take it, did not send it.
  Refused(Refusal),
  /// The team server already has `version` with these very steps, edited from the same version: an earlier send of them
  /// reached it and its answer did not come back. Nothing more was sent; the stream brings that version here.
  Reached { version: VersionNumber },
  /// The team server could not be reached, or nobody is signed in to it, or it did not answer in time: nothing more was
  /// sent.
  Unreached(Message),
  /// It failed otherwise. The server may have taken it: then it shows here once the team's changes are read.
  Failed(Message),
}

/// The code the team server answers for a pipeline it no longer has.
const GONE: &str = "not_found";

impl NotTaken {
  /// Why, in words: what the page says where a save's refusal shows, and what a tool answers.
  pub fn message(&self) -> Message {
    match self {
      Self::Changed { latest: Some(latest) } => {
        Message::new("pipelines.problems.teamChanged").with("version", latest.get().to_string())
      }
      Self::Changed { latest: None } => Message::new("pipelines.problems.teamRenameChanged"),
      Self::Reached { version } => {
        Message::new("pipelines.problems.teamAlreadySaved").with("version", version.get().to_string())
      }
      Self::Refused(Refusal::NotAdmin | Refusal::NotAdminHere) => Message::new("sharing.errors.notAdmin"),
      // A create that got no answer may have made the pipeline: the name is then taken by the person's own.
      Self::Refused(Refusal::NameTaken) => Message::new("pipelines.problems.teamNameTaken"),
      // The server no longer has the pipeline (`404 not_found`): said in words, not by its code.
      Self::Refused(Refusal::Other(code)) if code == GONE => Message::new("pipelines.problems.teamGone"),
      Self::Refused(refusal) => refusal.message(),
      Self::Unreached(reason) | Self::Failed(reason) => reason.clone(),
    }
  }

  /// The code a tool refuses with.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::Changed { .. } => "version_changed",
      Self::Reached { .. } => "already_on_server",
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
  /// version `based_on`; memory holds it once the server numbered it. That number.
  pub fn publish_team_pipeline(
    &self,
    repo_root: &Path,
    resource: ResourceId,
    based_on: VersionNumber,
    body: &Pipeline,
  ) -> Result<VersionNumber, NotTaken> {
    let (server, db) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let taken = |error: SharingError| not_taken(error, Some(&address));
    let team = pipeline_executors(self.team_config(repo_root)?.as_ref());
    let content = pipeline_content(&db, &team, resource, body).map_err(taken)?;
    let sent = content.body.clone();
    let publish = VersionPublish {
      idempotency_key: IdempotencyKey::generate(),
      based_on: Some(based_on),
      body: content.body,
      references: content.references,
    };
    let version = runtime()?
      .block_on(server.publish_version(resource, publish))
      .map_err(|error| match own_version(&error, &sent) {
        Some(version) => NotTaken::Reached { version },
        None => taken(error),
      })?;
    let number = version.number;
    self
      .inner
      .workspace
      .team
      .update(repo_root, |config| config.published(resource, version.into()));
    self.shared_changed();
    Ok(number)
  }

  /// Creates the team pipeline `name`, under `slug`, with `body` as its first version in the project at `repo_root`,
  /// and memory holds it once the server made it; its id.
  pub fn create_team_pipeline(
    &self,
    repo_root: &Path,
    (slug, name): (Slug, Name),
    body: &Pipeline,
  ) -> Result<ResourceId, NotTaken> {
    let (server, db) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let taken = |error: SharingError| not_taken(error, Some(&address));
    let id = ResourceId::generate();
    let team = pipeline_executors(self.team_config(repo_root)?.as_ref());
    let create = ResourceCreate {
      idempotency_key: IdempotencyKey::generate(),
      id,
      kind: SharedResourceKind::Pipeline,
      slug: slug.as_str().to_owned(),
      name: name.as_str().to_owned(),
      version: pipeline_content(&db, &team, id, body).map_err(taken)?,
    };
    let created = runtime()?.block_on(server.create_resource(create)).map_err(taken)?;
    if let Some(kept) = TeamResource::of(&created.resource, Some(created.version.into())) {
      self.inner.workspace.team.update(repo_root, |config| {
        config.keep(kept);
        true
      });
    }
    self.shared_changed();
    Ok(id)
  }

  /// Renames the team pipeline `resource` of the project at `repo_root` to `name` on the revision memory has of it;
  /// memory holds the name once the server took it.
  pub fn rename_team_pipeline(&self, repo_root: &Path, resource: ResourceId, name: &Name) -> Result<(), NotTaken> {
    let (server, _) = open(self, repo_root)?;
    let address = server.entry().address.clone();
    let Some(revision) = self.team_config(repo_root)?.live(resource).map(|held| held.revision) else {
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
    let (name, change) = (name.clone(), Changed::of(&renamed.updated));
    self
      .inner
      .workspace
      .team
      .update(repo_root, |config| config.rename(resource, name, change));
    self.shared_changed();
    Ok(())
  }

  /// The team's configuration of the project at `root` as this session read it; a change is not sent while it is being
  /// read or could not be read, since what the pipeline may run is told from it.
  fn team_config(&self, root: &Path) -> Result<std::sync::Arc<TeamConfig>, NotTaken> {
    self
      .inner
      .workspace
      .team
      .read(root)
      .loaded()
      .map_err(|unread| NotTaken::Unreached(unread.message()))
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

/// The version a `version_conflict` names when it holds the `sent` body itself: the person's own earlier send, which
/// reached the server though its answer did not come back.
fn own_version(error: &SharingError, sent: &serde_json::Value) -> Option<VersionNumber> {
  let SharingError::Client(client) = error else {
    return None;
  };
  match (client.code(), client.details()) {
    (Some(ErrorCode::VersionConflict), Some(SharingErrorDetails::Version { latest })) if latest.body == *sent => {
      Some(latest.number)
    }
    _ => None,
  }
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
    // The request went out: the server may have taken it.
    SharingError::Client(ClientError::TimedOut) => NotTaken::Unreached(Message::new("pipelines.problems.teamNoAnswer")),
    SharingError::Client(_) if error.unreached() => {
      NotTaken::Unreached(Message::new("pipelines.problems.teamUnreached"))
    }
    error if error.unreached() => NotTaken::Unreached(said(&error, address)),
    error => NotTaken::Failed(said(&error, address)),
  }
}
