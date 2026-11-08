//! An admin restores an archived team resource (`Agents · MCP server · admin · draft` A21 `yqYj9`: *Restore* brings the
//! resource back for everyone, and the draft kept here can be published again): the team server clears its archived
//! mark (`PATCH …/resources/{resource}`, made on the revision this computer has, once more on the revision the server
//! names when it moved on). The server's answer is the truth: once it says the resource is back, it is back here too
//! ([`ProjectDb::mark_shared_archived`]), without waiting for the stream to say the same. While the server cannot be
//! reached nothing is sent and the restore fails at once; it has no outbox.

use std::path::Path;

use orchester_api::{ErrorCode, ResourcePatch, SharedResource, SharingErrorDetails};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  time::Timestamp,
};
use orchester_store::{ProjectDb, ResourceSharing, StoreError};

use super::ProjectServer;
use crate::{
  background,
  messages::Message,
  sharing::{Servers, SharingError, team::said},
};

/// Why a team resource was not restored; nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotRestored {
  /// The team server refused it with its error `code`: `not_admin`, or `revision_conflict` when the resource changed
  /// again meanwhile.
  Refused { code: String },
  /// It could not be sent, e.g. the server cannot be reached.
  Failed(Message),
}

impl NotRestored {
  /// Why, in words, for the one place that says it.
  pub fn message(self) -> Message {
    match self {
      Self::Refused { code } if code == "not_admin" => Message::new("sharing.edit.restore.notAdmin"),
      Self::Refused { code } => Message::new("sharing.edit.restore.refused").with("code", code),
      Self::Failed(message) => message,
    }
  }
}

impl Servers {
  /// Restores the archived team resource `resource` of the project at `repo_root` on the team server.
  pub async fn restore_resource(&self, repo_root: &Path, resource: ResourceId) -> Result<(), NotRestored> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let (server, revision) = background::run(move || prepare(&servers, &root, resource))
      .await
      .map_err(NotRestored::Failed)??;
    let restored = match patch(&server, resource, revision).await {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::RevisionConflict) => {
        let Some(SharingErrorDetails::ResourceRevision { current }) = error.details() else {
          return Err(refused(&SharingError::Client(error)));
        };
        patch(&server, resource, current.revision).await
      }
      answer => answer,
    }
    .map_err(|error| refused(&error))?;
    let root = repo_root.to_owned();
    background::run(move || keep(&root, &restored))
      .await
      .map_err(NotRestored::Failed)?
      .map_err(NotRestored::Failed)?;
    self.shared_changed();
    Ok(())
  }
}

async fn patch(
  server: &ProjectServer,
  resource: ResourceId,
  revision: orchester_core::sharing::TaskRevision,
) -> Result<SharedResource, SharingError> {
  let patch = ResourcePatch {
    idempotency_key: IdempotencyKey::generate(),
    revision,
    name: None,
    archived: Some(false),
  };
  server
    .call(|client, token, project| async move { client.update_resource(&token, project, resource, &patch).await })
    .await
}

/// The server of the project at `root` and the revision this computer has of `resource`.
fn prepare(
  servers: &Servers,
  root: &Path,
  resource: ResourceId,
) -> Result<(ProjectServer, orchester_core::sharing::TaskRevision), NotRestored> {
  let failed = |error: &SharingError| NotRestored::Failed(said(error, None));
  let server = ProjectServer::open(servers, root)
    .map_err(|error| failed(&error))?
    .ok_or_else(|| failed(&SharingError::Store(StoreError::NotShared)))?;
  server.writable().map_err(|error| failed(&error))?;
  let db = ProjectDb::open(root).map_err(|error| failed(&error.into()))?;
  let Some(ResourceSharing::Shared(revision)) = db.resource_sharing(resource).map_err(|error| failed(&error.into()))?
  else {
    return Err(failed(&SharingError::Store(StoreError::ResourceNotFound(resource))));
  };
  Ok((server, revision))
}

/// Keeps the server's answer: the resource is back, at the revision the server gave it.
fn keep(root: &Path, restored: &SharedResource) -> Result<(), Message> {
  let failed = |error: &StoreError| crate::messages::store_error(error);
  let mut db = ProjectDb::open(root).map_err(|error| failed(&error))?;
  db.mark_shared_archived(restored.id, restored.archived, Timestamp::now())
    .map_err(|error| failed(&error))?;
  db.set_resource_sharing(restored.id, Some(ResourceSharing::Shared(restored.revision)))
    .map_err(|error| failed(&error))
}

fn refused(error: &SharingError) -> NotRestored {
  match error.code() {
    Some(code) => NotRestored::Refused { code: code_text(code) },
    None => NotRestored::Failed(said(error, None)),
  }
}

/// The server's error code as it writes it.
fn code_text(code: ErrorCode) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  quoted.trim_matches('"').to_owned()
}
