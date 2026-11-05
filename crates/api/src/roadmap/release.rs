//! A release as a team server keeps it under a revision, the writes of its definition, and every move of a task into
//! or out of one.

use chrono::NaiveDate;
use orchester_core::{
  id::{IdempotencyKey, ReleaseId, TaskId},
  name::Name,
  release::{ReleaseDetails, ReleaseState},
  run::CommitId,
  sharing::TaskRevision,
  task::organization::ChangeActor,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Attribution, UserRef, v2::nullable, wire};

/// What a person sets of a release; `planned` is a `YYYY-MM-DD` day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseFields {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub planned: Option<String>,
}

/// Fields that are not a release's details: a name that is not a name, or a planned day that is not `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a release's name is not a name or its planned day is not YYYY-MM-DD")]
pub struct ReleaseFieldsError;

impl ReleaseFields {
  /// The details these fields are, by core's rules.
  pub fn details(&self) -> Result<ReleaseDetails, ReleaseFieldsError> {
    let name = Name::new(&self.name).map_err(|_| ReleaseFieldsError)?;
    let planned = self
      .planned
      .as_deref()
      .map(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| ReleaseFieldsError))
      .transpose()?;
    Ok(ReleaseDetails::new(name, &self.description, planned))
  }

  pub fn from_core(details: &ReleaseDetails) -> Self {
    Self {
      name: details.name().as_str().to_owned(),
      description: details.description().to_owned(),
      planned: details.planned().map(|day| day.format("%Y-%m-%d").to_string()),
    }
  }
}

/// How a release was marked released: the `YYYY-MM-DD` day, who marked it, and the commit it shipped from when they
/// gave one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseMark {
  pub on: String,
  pub by: UserRef,
  pub commit: Option<CommitId>,
}

/// A release as the server keeps it. Its revision counts its changes, its changelog's too. `released` is `None` while
/// it is planned; once released it is final.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SharedReleaseWire", into = "SharedReleaseWire")]
pub struct SharedRelease {
  pub id: ReleaseId,
  pub fields: ReleaseFields,
  pub released: Option<ReleaseMark>,
  pub revision: TaskRevision,
  pub created: Attribution,
  pub updated: Attribution,
}

impl SharedRelease {
  pub const fn state(&self) -> ReleaseState {
    match self.released {
      Some(_) => ReleaseState::Released,
      None => ReleaseState::Planned,
    }
  }
}

/// A release whose `state` and `released_*` fields contradict each other.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error(
  "a released release names the day it was released and who marked it; a planned one names neither, nor a commit"
)]
pub struct ReleaseStateError;

/// Every field is sent; one without a value is `null`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedReleaseWire {
  id: ReleaseId,
  name: String,
  description: String,
  #[serde(deserialize_with = "nullable")]
  planned: Option<String>,
  state: ReleaseState,
  #[serde(deserialize_with = "nullable")]
  released_on: Option<String>,
  #[serde(deserialize_with = "nullable")]
  released_by: Option<UserRef>,
  #[serde(deserialize_with = "nullable")]
  released_commit: Option<CommitId>,
  revision: TaskRevision,
  created: Attribution,
  updated: Attribution,
}

impl TryFrom<SharedReleaseWire> for SharedRelease {
  type Error = ReleaseStateError;

  fn try_from(wire: SharedReleaseWire) -> Result<Self, Self::Error> {
    let released = match (wire.state, wire.released_on, wire.released_by, wire.released_commit) {
      (ReleaseState::Planned, None, None, None) => None,
      (ReleaseState::Released, Some(on), Some(by), commit) => Some(ReleaseMark { on, by, commit }),
      _ => return Err(ReleaseStateError),
    };
    Ok(Self {
      id: wire.id,
      fields: ReleaseFields {
        name: wire.name,
        description: wire.description,
        planned: wire.planned,
      },
      released,
      revision: wire.revision,
      created: wire.created,
      updated: wire.updated,
    })
  }
}

impl From<SharedRelease> for SharedReleaseWire {
  fn from(release: SharedRelease) -> Self {
    let state = release.state();
    let (released_on, released_by, released_commit) = match release.released {
      Some(mark) => (Some(mark.on), Some(mark.by), mark.commit),
      None => (None, None, None),
    };
    Self {
      id: release.id,
      name: release.fields.name,
      description: release.fields.description,
      planned: release.fields.planned,
      state,
      released_on,
      released_by,
      released_commit,
      revision: release.revision,
      created: release.created,
      updated: release.updated,
    }
  }
}

/// A task moved into a release, out of one, or from one to another; `sequence` orders every move of the server's.
/// *Mark released* and deleting a release move tasks too, with `actor` `orchester` and `person` respectively.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseChange {
  pub sequence: u64,
  pub task: TaskId,
  #[serde(deserialize_with = "nullable")]
  pub from: Option<ReleaseId>,
  #[serde(deserialize_with = "nullable")]
  pub to: Option<ReleaseId>,
  pub by: UserRef,
  #[serde(with = "wire::Actor")]
  pub actor: ChangeActor,
  pub at: i64,
}

/// `POST …/releases`: a planned release with the id the desktop made for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCreate {
  pub idempotency_key: IdempotencyKey,
  pub id: ReleaseId,
  pub release: ReleaseFields,
}

/// `PATCH …/releases/{release}`: the release's fields as a whole, made on `revision`. A released one is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasePatch {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
  pub release: ReleaseFields,
}

/// `POST …/releases/{release}/released`: *Mark released* on `revision`, with the commit it shipped from or `null`. The
/// server records the day and who marked it, keeps the release's Done tasks and moves every other one but the Canceled
/// to the next planned release, or out of any release when there is none; it answers [`ReleaseMarked`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkReleased {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
  #[serde(deserialize_with = "nullable")]
  pub commit: Option<CommitId>,
}

/// `DELETE …/releases/{release}`: deletes a planned release made on `revision`, with its changelog; its tasks are left
/// without a release. It answers [`ReleaseDeleted`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseDelete {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseResponse {
  pub release: SharedRelease,
}

/// The answer of [`MarkReleased`]: the release as released, and the moves it made, in ascending `sequence`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseMarked {
  pub release: SharedRelease,
  pub moves: Vec<ReleaseChange>,
}

/// The answer of [`ReleaseDelete`]: each task taken out of the release, in ascending `sequence`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseDeleted {
  pub moves: Vec<ReleaseChange>,
}

/// The `reason` of `422 release`: the release is released, so it no longer changes, or a task names a release the
/// project does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRefusal {
  Released,
  ReleaseNotFound,
}

impl ReleaseRefusal {
  /// The reason as `SharingErrorDetails::Reason` carries it.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Released => "released",
      Self::ReleaseNotFound => "release_not_found",
    }
  }
}
