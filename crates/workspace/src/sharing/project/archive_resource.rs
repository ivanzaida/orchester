//! An admin archives a team resource (`docs/design/team-configuration.md`, decision 11) or restores one (`Agents ·
//! Archived · admin · Restore` A36 `qvMZD`): the team server sets or clears its archived mark (`PATCH
//! …/resources/{resource}`, made on the revision this computer has) and every computer follows through its event
//! stream, where the runs that pin an archived resource stop ([`crate::sharing::sync`]). The revision is the one the
//! team's configuration in memory holds ([`crate::team_config`]), which keeps the server's answer at once, so a restore
//! made after this computer's own archive is made on the revision that archive left. The stream's `resource.archived`
//! is what stops the runs, and it stops them only for a mark that is new. While the server cannot be reached nothing is
//! sent and the change fails at once; it has no outbox. Archiving stops work on every computer, so it is a person's
//! alone: automation is refused before anything is sent. Restoring loses nothing and stops nothing.

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

/// Why an archived team resource was not restored; nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotRestored {
  /// The team server refused it with its error `code`: `not_admin`, or `revision_conflict` when the resource changed
  /// since this computer read it.
  Refused { code: String },
  /// It could not be sent, e.g. the server cannot be reached.
  Failed(Message),
}

impl From<NotMarked> for NotRestored {
  fn from(not: NotMarked) -> Self {
    match not {
      NotMarked::Refused { code } => Self::Refused { code },
      NotMarked::Failed(message) => Self::Failed(message),
    }
  }
}

impl From<NotMarked> for NotArchived {
  fn from(not: NotMarked) -> Self {
    match not {
      NotMarked::Refused { code } => Self::Refused { code },
      NotMarked::Failed(message) => Self::Failed(message),
    }
  }
}

/// Why the archived mark was not changed.
enum NotMarked {
  Refused { code: String },
  Failed(Message),
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
    self
      .mark_archived(repo_root, resource, true)
      .await
      .map_err(NotArchived::from)
  }

  /// Restores the archived team resource `resource` of the project at `repo_root` on the team server: back in every
  /// list and picker for the whole team, as it was. The runs its archive stopped stay stopped.
  pub async fn restore_resource(&self, repo_root: &Path, resource: ResourceId) -> Result<(), NotRestored> {
    self
      .mark_archived(repo_root, resource, false)
      .await
      .map_err(NotRestored::from)
  }

  /// Sets the archived mark of `resource` to `archived` on the team server, and keeps its answer in the team's
  /// configuration in memory.
  async fn mark_archived(&self, repo_root: &Path, resource: ResourceId, archived: bool) -> Result<(), NotMarked> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let prepared = background::run(move || prepare(&servers, &root, resource, archived))
      .await
      .map_err(NotMarked::Failed)?;
    let (server, patch) = prepared?;
    match server
      .call(|client, token, project| async move { client.update_resource(&token, project, resource, &patch).await })
      .await
    {
      Ok(answer) => {
        self
          .inner
          .workspace
          .team
          .update(repo_root, |config| config.changed(&answer));
        self.shared_changed();
        Ok(())
      }
      Err(error) => Err(match error.code() {
        Some(code) => NotMarked::Refused { code: code_text(code) },
        None => NotMarked::Failed(said(&error, None)),
      }),
    }
  }
}

/// The server of the project at `root` and the change of `resource`'s archived mark to `archived` on the revision this
/// computer has of it.
fn prepare(
  servers: &Servers,
  root: &Path,
  resource: ResourceId,
  archived: bool,
) -> Result<(ProjectServer, ResourcePatch), NotMarked> {
  let failed = |error: &SharingError| NotMarked::Failed(said(error, None));
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
    archived: Some(archived),
  };
  Ok((server, patch))
}

/// The server's error code as it writes it.
fn code_text(code: ErrorCode) -> String {
  let quoted = serde_json::to_string(&code).expect("an error code serializes as a string");
  quoted.trim_matches('"').to_owned()
}
