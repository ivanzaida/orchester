//! An admin archives a team resource (`docs/design/team-configuration.md`, decision 11): the team server marks it
//! archived (`PATCH …/resources/{resource}`, made on the revision this computer has) and every computer follows through
//! its event stream, where the runs that pin the resource stop ([`crate::sharing::sync`]). The revision is the one the
//! team's configuration in memory holds ([`crate::team_config`]). Nothing is marked here
//! first: the stream's `resource.archived` is what stops the runs, and it stops them only for a mark that is new.
//! While the server cannot be reached nothing is sent and the archive fails at once; it has no outbox. It stops work on
//! every computer, so it is a person's alone: automation is refused before anything is sent.

use std::path::Path;

use orchester_api::{ErrorCode, ResourcePatch};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  run::Answerer,
};
use orchester_store::StoreError;

use super::ProjectServer;
use crate::{
  background,
  messages::Message,
  sharing::{Servers, SharingError, team::said},
};

/// Why a team resource was not archived; nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotArchived {
  /// The team server refused it with its error `code`: `not_admin`, or `revision_conflict` when the resource changed
  /// since this computer read it.
  Refused { code: String },
  /// It could not be sent, e.g. the server cannot be reached.
  Failed(Message),
  /// Automation asked: it stops runs on every computer, so only a person archives. Nothing was sent.
  PersonOnly,
}

impl NotArchived {
  /// What a person-only refusal says.
  pub fn person_only_message() -> Message {
    Message::new("appMcp.errors.teamArchivePersonOnly")
  }
}

impl Servers {
  /// Archives the team resource `resource` of the project at `repo_root` on the team server, as `by` asks: automation
  /// is refused ([`NotArchived::PersonOnly`]) without reading or sending anything.
  pub async fn archive_resource(
    &self,
    repo_root: &Path,
    resource: ResourceId,
    by: Answerer,
  ) -> Result<(), NotArchived> {
    if by == Answerer::Automation {
      return Err(NotArchived::PersonOnly);
    }
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let prepared = background::run(move || prepare(&servers, &root, resource))
      .await
      .map_err(NotArchived::Failed)?;
    let (server, patch) = prepared?;
    match server
      .call(|client, token, project| async move { client.update_resource(&token, project, resource, &patch).await })
      .await
    {
      Ok(_) => Ok(()),
      Err(error) => Err(match error.code() {
        Some(code) => NotArchived::Refused { code: code_text(code) },
        None => NotArchived::Failed(said(&error, None)),
      }),
    }
  }
}

/// The server of the project at `root` and the archive of `resource` on the revision this computer has of it.
fn prepare(
  servers: &Servers,
  root: &Path,
  resource: ResourceId,
) -> Result<(ProjectServer, ResourcePatch), NotArchived> {
  let failed = |error: &SharingError| NotArchived::Failed(said(error, None));
  let server = ProjectServer::open(servers, root)
    .map_err(|error| failed(&error))?
    .ok_or_else(|| failed(&SharingError::Store(StoreError::NotShared)))?;
  server.writable().map_err(|error| failed(&error))?;
  let revision = servers
    .inner
    .workspace
    .team
    .read(root)
    .config()
    .and_then(|config| config.resource(resource).map(|found| found.revision))
    .ok_or_else(|| failed(&SharingError::Store(StoreError::ResourceNotFound(resource))))?;
  let patch = ResourcePatch {
    idempotency_key: IdempotencyKey::generate(),
    revision,
    name: None,
    archived: Some(true),
  };
  Ok((server, patch))
}

/// The server's error code as it writes it.
fn code_text(code: ErrorCode) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  quoted.trim_matches('"').to_owned()
}
