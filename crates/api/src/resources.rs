//! A shared project's configuration (`docs/architecture/server.md`, "Shared configuration", S5): its resources
//! (pipelines, agent profiles, instructions, skills and tools), their published versions, and what a version names that
//! each computer binds to its own connection, MCP server or secret. A skill's parts travel apart from its body, by
//! their hash (`PUT …/files/{sha256}`). Only admins write (row 72); a member's write answers `403 not_admin`.

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, IdempotencyKey, ResourceId, SecretId},
  resource::{ResourceKind, VersionNumber},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use crate::Attribution;

/// The kinds of resource a server project shares: every kind of core's but the test suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedResourceKind {
  Pipeline,
  AgentProfile,
  Instruction,
  Skill,
  Tool,
}

impl SharedResourceKind {
  pub const fn core(self) -> ResourceKind {
    match self {
      Self::Pipeline => ResourceKind::Pipeline,
      Self::AgentProfile => ResourceKind::AgentProfile,
      Self::Instruction => ResourceKind::Instruction,
      Self::Skill => ResourceKind::Skill,
      Self::Tool => ResourceKind::Tool,
    }
  }

  /// The shared kind of a core kind; `None` for a test suite, a service or a stack, which stay local.
  pub const fn from_core(kind: ResourceKind) -> Option<Self> {
    match kind {
      ResourceKind::Pipeline => Some(Self::Pipeline),
      ResourceKind::AgentProfile => Some(Self::AgentProfile),
      ResourceKind::Instruction => Some(Self::Instruction),
      ResourceKind::Skill => Some(Self::Skill),
      ResourceKind::Tool => Some(Self::Tool),
      ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => None,
    }
  }
}

/// A resource as the server keeps it (`Resource`). Its id is the sharer's, kept on upload; its revision counts the
/// changes of its name and archived mark as a task's does; `latest` is its newest version's number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedResource {
  pub id: ResourceId,
  pub kind: SharedResourceKind,
  pub slug: String,
  pub name: String,
  pub archived: bool,
  pub revision: TaskRevision,
  pub latest: Option<VersionNumber>,
  pub created: Attribution,
  pub updated: Attribution,
}

/// What a reference names on the publisher's computer: a connection, an MCP server or a secret by its id there, or a
/// shared tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReferenceTarget {
  Connection { id: ConnectionId },
  McpServer { id: ConnectionId },
  Secret { id: SecretId },
  Tool { id: ResourceId },
}

/// One id a version's body names that is not a shared resource of the project, with what each computer binds it by:
/// its name, and its role, the connection's kind (`claude_cli`, `openai_api`, …) or the secret's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
  #[serde(flatten)]
  pub target: ReferenceTarget,
  pub name: String,
  pub role: Option<String>,
}

/// One published version of a resource (`Version`), immutable and numbered by the server. `body` is the kind's body as
/// core stores it, `hash` core's content hash of it, `based_on` the version it was edited from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedVersion {
  pub resource: ResourceId,
  pub number: VersionNumber,
  pub body: serde_json::Value,
  pub hash: ContentHash,
  pub references: Vec<Reference>,
  pub based_on: Option<VersionNumber>,
  pub published: Attribution,
}

/// A version's content as an admin sends it: the body, and a reference for every id it names that is not a shared
/// resource (`422 invalid { reason: "references" }` otherwise).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionContent {
  pub body: serde_json::Value,
  pub references: Vec<Reference>,
}

/// `POST …/resources`: a resource with the id the desktop made for it, and its first version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceCreate {
  pub idempotency_key: IdempotencyKey,
  pub id: ResourceId,
  pub kind: SharedResourceKind,
  pub slug: String,
  pub name: String,
  pub version: VersionContent,
}

/// The answer to `POST …/resources`: the resource and its v1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCreated {
  pub resource: SharedResource,
  pub version: SharedVersion,
}

/// One version of a local resource uploaded with its number and when it was published here; the server names the
/// uploader as its publisher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadVersion {
  pub number: VersionNumber,
  pub body: serde_json::Value,
  pub references: Vec<Reference>,
  pub published_at: i64,
}

/// A local resource uploaded with its history when a project is shared (`UploadResource`): its id and version
/// numbers are kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadResource {
  pub id: ResourceId,
  pub kind: SharedResourceKind,
  pub slug: String,
  pub name: String,
  pub archived: bool,
  pub versions: Vec<UploadVersion>,
}

/// `POST …/resources/import`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcesImport {
  pub idempotency_key: IdempotencyKey,
  pub resources: Vec<UploadResource>,
}

/// The answer to `POST …/resources/import`: the uploaded resources as the server keeps them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcesImported {
  pub resources: Vec<SharedResource>,
}

/// `POST …/resources/{resource}/versions`: a draft published on the version it was edited from. `409
/// version_conflict` carries the latest version when that is no longer `based_on`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionPublish {
  pub idempotency_key: IdempotencyKey,
  pub based_on: Option<VersionNumber>,
  pub body: serde_json::Value,
  pub references: Vec<Reference>,
}

/// `PATCH …/resources/{resource}`: a rename or an archived mark, made on `revision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePatch {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub archived: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceResponse {
  pub resource: SharedResource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionResponse {
  pub version: SharedVersion,
}

/// `GET …/resources`: every resource of the project and the event cursor the list is current at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
  pub resources: Vec<SharedResource>,
  pub cursor: u64,
}

/// `GET …/resources/{resource}/versions?after=`: the versions after `after`, oldest first, at most 64 or 1 MiB; `next`
/// is the `after` of the following page when there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
  pub versions: Vec<SharedVersion>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub next: Option<VersionNumber>,
}

/// How `PUT …/files/{sha256}` kept a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStored {
  /// `201`: the server did not have it.
  Created,
  /// `200`: the server had it already.
  Known,
}
