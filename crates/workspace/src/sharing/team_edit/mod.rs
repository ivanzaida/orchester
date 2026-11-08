//! An admin's edit of a team resource, published from the hub's panel (`Agents · MCP server · admin · draft` A13
//! `WMtxm`, `docs/design/team-configuration.md` decision 6): the draft becomes the next version and is sent to the team
//! server at once, and the answer is the panel's, in place. The version counts as published only once the server has
//! numbered it ([`Edited::Published`]). A server that cannot be reached leaves it in the outbox, sent when it answers
//! ([`Edited::Waiting`], A16 `x3ywL`; cancelling takes it back as a draft); one that refuses it takes it back as the
//! draft, which is kept, and says why ([`EditRefused`]). A send that failed on this computer after the server may have
//! answered leaves the version in the outbox, whose retry is idempotent, and says so. One send of a project's outbox at
//! a time ([`Servers::send_lane`]). Nothing here is merged with a version someone published meanwhile: that is the
//! person's, field by field ([`crate::agents::team_mcp_conflict`]).

mod refusal;
mod reopen;

use std::path::Path;

use orchester_core::{
  id::{ResourceId, VersionId},
  resource::{PublishError, TeamMcpServer, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState, StoreError, WaitingPublish};

pub(super) use self::refusal::recorded;
pub use self::refusal::{Local, Refusal, Rejection};
use super::{
  Servers, SharingError, project::ProjectServer, send_lane::send_turn, sync::RemoteResourceStore, team::said,
};
use crate::{
  background,
  messages::{self, Message},
};

/// How a published edit stands once the team server was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edited {
  /// The server took it as this version.
  Published(VersionNumber),
  /// The server could not be reached: the version waits in the outbox and is sent when it answers.
  Waiting(VersionId),
}

/// Why an edit is not published; the draft is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditRefused {
  /// The team server, or this computer, which knows the team cannot take it, refused it.
  Refused(Refusal),
  /// Someone published `latest` from the same version first; the draft is now edited from it.
  Changed { latest: VersionNumber },
  /// It could not be sent for a reason other than being offline. A version that was already published waits in the
  /// outbox all the same, and is shown there.
  Failed(Message),
}

impl EditRefused {
  /// What a press that only a person may make says when automation made it: nothing was sent.
  pub fn person_only() -> Self {
    Self::Failed(Self::person_only_message())
  }

  /// What a press that only a person may make says when automation made it, for a caller that has no [`EditRefused`].
  pub fn person_only_message() -> Message {
    Message::new("sharing.errors.personOnly")
  }

  /// Why, in words, for a caller that has one place to say it.
  pub fn message(&self) -> Message {
    match self {
      Self::Refused(refusal) => refusal.message(),
      Self::Changed { latest } => Refusal::Changed.message().with("version", latest.get().to_string()),
      Self::Failed(message) => message.clone(),
    }
  }
}

impl Servers {
  /// Publishes the draft of the team MCP server `resource` of the project at `repo_root` as its next version and sends
  /// it. A local project's has nothing to send, so it is published at once.
  pub async fn publish_mcp_edit(&self, repo_root: &Path, resource: ResourceId) -> Result<Edited, EditRefused> {
    let (servers, root) = (self.clone(), repo_root.to_owned());
    let settled = background::run(move || settle(&servers, &root, resource))
      .await
      .map_err(EditRefused::Failed)?;
    // How the team's resources stand here changed: a version waits for the server, or the draft came back.
    self.shared_changed();
    settled
  }
}

fn settle(servers: &Servers, root: &Path, resource: ResourceId) -> Result<Edited, EditRefused> {
  let failed = |error: &StoreError| EditRefused::Failed(messages::store_error(error));
  // Nothing else sends or takes back what waits meanwhile: a cancel would delete a version the server then numbers.
  let lane = servers.send_lane(root);
  let _sending = send_turn(&lane);
  let mut db = ProjectDb::open(root).map_err(|error| failed(&error))?;
  if unsent(&db, resource)
    .map_err(|error| failed(&error))?
    .is_some_and(|first| first.state != PublishState::Waiting)
  {
    return Err(blocked());
  }
  let version = match db.publish_draft::<TeamMcpServer>(resource, Timestamp::now()) {
    Ok(version) => version.id(),
    Err(StoreError::NotAdmin) => return Err(EditRefused::Refused(Refusal::NotAdminHere)),
    // The team published since the draft was edited, and this computer has read it: nothing was sent, and the draft is
    // kept as it is, edited from the version it was edited from.
    Err(StoreError::Publish(PublishError::StaleDraft {
      latest: Some(latest), ..
    })) => return Err(EditRefused::Changed { latest }),
    Err(error) => return Err(failed(&error)),
  };
  let sent = send(servers, root, resource);
  let state = waiting_state(&db, version).map_err(|error| failed(&error))?;
  match (state, sent) {
    (None, _) => confirmed(&db, resource),
    (Some(PublishState::Waiting), Ok(())) => queued(&db, resource, version),
    (Some(PublishState::Waiting), Err(error)) if error.unreached() => Ok(Edited::Waiting(version)),
    // The server may have answered: a retry is idempotent, so the version stays in the outbox, and what went wrong is
    // told. Taking it back here could delete a version the server holds.
    (Some(PublishState::Waiting), Err(error)) => Err(EditRefused::Failed(said(&error, None))),
    (Some(PublishState::Refused { code }), _) => {
      taken_back(&mut db, version, EditRefused::Refused(Refusal::of_code(&code)))
    }
    (Some(PublishState::Conflict { latest }), _) => taken_back(&mut db, version, EditRefused::Changed { latest }),
  }
}

/// A send that did not take `version` and did not fail: it waits, unless an earlier version of the resource, which a
/// send stops at, was not taken meanwhile.
fn queued(db: &ProjectDb, resource: ResourceId, version: VersionId) -> Result<Edited, EditRefused> {
  match unsent(db, resource).map_err(|error| EditRefused::Failed(messages::store_error(&error)))? {
    Some(first) if first.version != version => Err(blocked()),
    _ => Ok(Edited::Waiting(version)),
  }
}

fn blocked() -> EditRefused {
  EditRefused::Failed(Message::new("sharing.edit.blocked"))
}

/// Nothing shows as published: the version the server did not take goes back to being the draft, which is kept, still
/// edited from the version it was edited from: when the team published meanwhile, the draft says so.
fn taken_back(db: &mut ProjectDb, version: VersionId, refused: EditRefused) -> Result<Edited, EditRefused> {
  db.reopen_draft_on_its_base::<TeamMcpServer>(version, Timestamp::now())
    .map_err(|error| EditRefused::Failed(messages::store_error(&error)))?;
  Err(refused)
}

/// The first version of `resource` that waits for the server, oldest first.
fn unsent(db: &ProjectDb, resource: ResourceId) -> Result<Option<WaitingPublish>, StoreError> {
  Ok(
    db.waiting_publishes()?
      .into_iter()
      .find(|waiting| waiting.resource == resource),
  )
}

/// The number the server gave the version just published: the newest this computer keeps.
fn confirmed(db: &ProjectDb, resource: ResourceId) -> Result<Edited, EditRefused> {
  db.latest_version::<TeamMcpServer>(resource)
    .map_err(|error| EditRefused::Failed(messages::store_error(&error)))?
    .map(|version| Edited::Published(version.number()))
    .ok_or_else(|| EditRefused::Failed(messages::interrupted()))
}

/// Sends the waiting versions of `resource` to its server; nothing for a local project.
fn send(servers: &Servers, root: &Path, resource: ResourceId) -> Result<(), SharingError> {
  let Some(server) = ProjectServer::open(servers, root)? else {
    return Ok(());
  };
  RemoteResourceStore::new(ProjectDb::open(root)?, server)?.send_created(resource)
}

/// How `version`, published here, stands with the team server; `None` once the server numbered it.
fn waiting_state(db: &ProjectDb, version: VersionId) -> Result<Option<PublishState>, StoreError> {
  Ok(
    db.waiting_publishes()?
      .into_iter()
      .find(|waiting| waiting.version == version)
      .map(|waiting| waiting.state),
  )
}
