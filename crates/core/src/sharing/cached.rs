//! A shared task as the desktop caches it (`docs/architecture/server.md`, "Desktop data"): what the server confirmed.
//! The store writes it; [`super::delta`] moves it along the project's event stream.

use std::num::NonZeroU32;

use super::{ClaimKind, TaskRevision};
use crate::{
  id::{ClaimId, ModuleId, TaskId, UserId},
  name::Name,
  run::TaskPhase,
  task::{TaskLimits, TaskStatus, organization::TaskOrganization},
  time::Timestamp,
};

/// A member of the server project, as the cache names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedPerson {
  pub id: UserId,
  pub name: String,
}

/// Who holds a shared task, as last read from the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedClaim {
  pub id: ClaimId,
  pub holder: SharedPerson,
  pub kind: ClaimKind,
  pub since: Timestamp,
  pub heartbeat_at: Timestamp,
  pub stale: bool,
  pub taken_over_from: Option<SharedPerson>,
}

/// The short status of a shared task's latest run that its holder reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedRunStatus {
  pub holder: SharedPerson,
  pub number: NonZeroU32,
  pub phase: TaskPhase,
  pub step: Option<String>,
  pub started_at: Timestamp,
  pub phase_since: Timestamp,
  pub updated_at: Timestamp,
}

/// A shared task as the server confirmed it. Its pipeline is a slug: pipelines are not shared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTask {
  pub id: TaskId,
  pub number: NonZeroU32,
  pub revision: TaskRevision,
  pub title: Name,
  pub brief: String,
  pub criteria: Vec<String>,
  pub pipeline: Option<String>,
  pub limits: TaskLimits,
  pub status: TaskStatus,
  pub organization: TaskOrganization,
  /// The module the task is in; the cache has it.
  pub module: Option<ModuleId>,
  pub former_key: Option<String>,
  pub created_by: String,
  pub created_at: Timestamp,
  pub updated_by: String,
  pub updated_at: Timestamp,
  pub claim: Option<CachedClaim>,
  pub run: Option<CachedRunStatus>,
}
