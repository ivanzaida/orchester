use orchester_core::{
  id::{IdempotencyKey, ProjectId, TagId, UserId},
  repository::RepositoryIdentity,
};
use serde::{Deserialize, Serialize};

use crate::{DroppedLink, SharedTask, UploadTask};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRef {
  pub id: UserId,
  pub display_name: String,
}

pub use orchester_core::sharing::MemberRole;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedProject {
  pub id: ProjectId,
  /// Unique on the server and fixed once created; the last segment of the project's address.
  pub slug: String,
  /// The project's address, its identity: the server's address and the slug.
  pub address: String,
  pub key: String,
  pub name: String,
  /// The repository its creator or first linked checkout had; compared with a checkout's `origin` when it is linked.
  pub remote_hint: Option<RepositoryIdentity>,
  pub role: Option<MemberRole>,
  pub next_number: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
  pub user: UserRef,
  #[serde(default)]
  pub username: String,
  pub role: MemberRole,
  pub added_at: i64,
}

/// A project as the server admin manages it: admins see every project, owners theirs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedProject {
  #[serde(flatten)]
  pub project: SharedProject,
  pub members: u32,
  pub open_invites: u32,
  pub owners: Vec<UserRef>,
  pub created_at: i64,
  pub created_by: UserRef,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagedProjects {
  pub projects: Vec<ManagedProject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagedProjectResponse {
  pub project: ManagedProject,
}

/// API 2's project creation by repository, replaced in API 3 by [`NewProject`]: the server refuses it. Kept while the
/// workspace's Share, which the frozen `crates/desktop` calls, still sends it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCreate {
  pub idempotency_key: IdempotencyKey,
  pub name: String,
  pub key: String,
  /// Untrusted identity or fetch URL; may contain credentials. This request has no Debug.
  pub remote: String,
  pub tasks: Vec<UploadTask>,
}

/// `POST /projects` (admins): a project by name and slug; the caller becomes its owner.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewProject {
  pub idempotency_key: IdempotencyKey,
  pub name: String,
  pub slug: String,
  /// The task key; without one the server suggests it from the name.
  #[serde(default)]
  pub key: Option<String>,
  /// Untrusted identity or fetch URL; may contain credentials. This request has no Debug.
  pub remote_hint: Option<String>,
  #[serde(default)]
  pub tasks: Vec<UploadTask>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectChanges {
  pub name: Option<String>,
  pub key: Option<String>,
  /// Untrusted identity or fetch URL; may contain credentials. An empty one clears the hint.
  pub remote_hint: Option<String>,
}

/// `POST /projects/{id}/link`: a checkout linked to the project uploads its tasks and its modules. Into a project with
/// no tasks they keep their numbers; otherwise they are imported with new ones.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectLink {
  pub idempotency_key: IdempotencyKey,
  pub tasks: Vec<UploadTask>,
  /// The checkout's `origin`, the project's `remote_hint` while it has none. Untrusted; may contain credentials.
  pub remote: Option<String>,
  #[serde(default)]
  pub modules: Vec<crate::UploadModule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectLinked {
  pub tasks: Vec<SharedTask>,
  pub dropped_links: Vec<DroppedLink>,
  pub cursor: u64,
  /// The project's modules after the upload, and the uploaded ones that became a module of their name.
  #[serde(default)]
  pub modules: Vec<crate::SharedModule>,
  #[serde(default)]
  pub mapped: Vec<crate::MappedModule>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberCreate {
  pub username: String,
  pub role: MemberRole,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberChanges {
  pub role: MemberRole,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagName {
  pub name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagMerge {
  pub into: TagId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedTag {
  pub id: TagId,
  pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Projects {
  pub projects: Vec<SharedProject>,
}
#[derive(Serialize, Deserialize)]
pub struct ProjectResponse {
  pub project: SharedProject,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectCreated {
  pub project: SharedProject,
  pub tasks: Vec<SharedTask>,
  pub cursor: u64,
  pub dropped_links: Vec<DroppedLink>,
}
#[derive(Serialize, Deserialize)]
pub struct Members {
  pub members: Vec<Member>,
}
#[derive(Serialize, Deserialize)]
pub struct MemberResponse {
  pub member: Member,
}
#[derive(Serialize, Deserialize)]
pub struct Tags {
  pub tags: Vec<SharedTag>,
}
#[derive(Serialize, Deserialize)]
pub struct TagResponse {
  pub tag: SharedTag,
}
#[derive(Serialize, Deserialize)]
pub struct TagsChanged {
  pub tags: Vec<SharedTag>,
  pub tasks: Vec<SharedTask>,
}
