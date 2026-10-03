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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
  Owner,
  Member,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedProject {
  pub id: ProjectId,
  pub key: String,
  pub name: String,
  pub remote: RepositoryIdentity,
  pub role: MemberRole,
  pub next_number: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
  pub user: UserRef,
  pub role: MemberRole,
  pub added_at: i64,
}

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

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectChanges {
  pub name: Option<String>,
  pub key: Option<String>,
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
#[derive(Serialize, Deserialize)]
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
