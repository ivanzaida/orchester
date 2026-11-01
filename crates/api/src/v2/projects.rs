use orchester_core::{id::ProjectId, repository::RepositoryIdentity};
use serde::{Deserialize, Serialize};

use super::MemberRole;
use crate::UserRef;

/// A workspace keeps its project identity. Only its role contract changes on the new API surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedProject {
  pub id: ProjectId,
  pub slug: String,
  pub address: String,
  pub key: String,
  pub name: String,
  pub remote_hint: Option<RepositoryIdentity>,
  pub role: Option<MemberRole>,
  pub next_number: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedProject {
  #[serde(flatten)]
  pub project: SharedProject,
  pub members: u32,
  pub open_invites: u32,
  /// Workspace-admin memberships; server admins are separately named by `Members.admins`.
  pub admins: Vec<UserRef>,
  pub created_at: i64,
  pub created_by: UserRef,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Projects {
  pub projects: Vec<SharedProject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectResponse {
  pub project: SharedProject,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagedProjects {
  pub projects: Vec<ManagedProject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagedProjectResponse {
  pub project: ManagedProject,
}
