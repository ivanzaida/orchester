//! A shared project's configuration (`docs/architecture/server.md`, "Shared configuration", S5): its resources
//! (pipelines, agent profiles, instructions, skills, tools, MCP servers and team reviewers) and their published
//! versions. A resource refers only to other team resources by server id and to secrets by name
//! ([`ReferenceTarget`]); what is local by nature stays on each computer. A skill's parts travel apart from its body,
//! by hash (`PUT …/files/{sha256}`). Members read; only the project's owner or a server admin writes; a member's write
//! answers `403 not_admin`.

use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{IdempotencyKey, ResourceId},
  resource::{ResourceKind, TeamAgentProfile, TeamReviewer, TeamTool, VersionNumber},
  secret::SecretName,
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use crate::Attribution;

/// The kinds of resource a server project shares: every kind of core's but agent testing's (test suite, service,
/// stack).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedResourceKind {
  Pipeline,
  AgentProfile,
  Instruction,
  Skill,
  Tool,
  /// Body: [`orchester_core::resource::TeamReviewer`], which names a CLI kind (a decision reviewer nothing), never a
  /// connection, a local id or a secret.
  TeamReviewer,
  /// Body: [`orchester_core::resource::TeamMcpServer`], with no program path, variable or secret value, or local id.
  McpServer,
}

impl SharedResourceKind {
  pub const fn core(self) -> ResourceKind {
    match self {
      Self::Pipeline => ResourceKind::Pipeline,
      Self::AgentProfile => ResourceKind::AgentProfile,
      Self::Instruction => ResourceKind::Instruction,
      Self::Skill => ResourceKind::Skill,
      Self::Tool => ResourceKind::Tool,
      Self::TeamReviewer => ResourceKind::TeamReviewer,
      Self::McpServer => ResourceKind::McpServer,
    }
  }

  /// The shared kind of a core kind; `None` for agent testing's test suites, services and stacks, which stay local.
  pub const fn from_core(kind: ResourceKind) -> Option<Self> {
    match kind {
      ResourceKind::Pipeline => Some(Self::Pipeline),
      ResourceKind::AgentProfile => Some(Self::AgentProfile),
      ResourceKind::Instruction => Some(Self::Instruction),
      ResourceKind::Skill => Some(Self::Skill),
      ResourceKind::Tool => Some(Self::Tool),
      ResourceKind::TeamReviewer => Some(Self::TeamReviewer),
      ResourceKind::McpServer => Some(Self::McpServer),
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

/// What a team resource's version names besides itself (`docs/design/team-configuration.md`): other team resources by
/// their server id, secrets by name, and the CLI kind a profile or language reviewer runs with. It never holds an id
/// or a path from a computer: no connection, no local MCP server, no secret id. Each computer fills in what is local
/// by nature.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceTarget {
  /// The agent provider a profile's or language reviewer's connection must have; each computer picks one of its own
  /// connections of that kind.
  CliKind { cli: AgentProvider },
  /// A team resource (an MCP server, tool, skill, instruction or profile) by its id on the server.
  TeamResource { kind: SharedResourceKind, id: ResourceId },
  /// A secret by its name; its value stays on each computer.
  SecretName { name: SecretName },
}

impl ReferenceTarget {
  /// The references of a team agent profile's body, the one list a version of it sends: its CLI kind, then its MCP
  /// servers, tools, skills and instructions as team resources, then its secrets by name, each in the body's order. The
  /// body is the source; the references repeat what it names so the server can check the ids exist and read what uses
  /// what without parsing every kind, and a version whose references differ from this list is not the body's.
  pub fn of_profile(profile: &TeamAgentProfile) -> Vec<Self> {
    let resources =
      |kind, ids: &[ResourceId]| -> Vec<Self> { ids.iter().map(|id| Self::TeamResource { kind, id: *id }).collect() };
    let mut references = vec![Self::CliKind { cli: profile.cli() }];
    references.extend(resources(SharedResourceKind::McpServer, profile.mcp_servers()));
    references.extend(resources(SharedResourceKind::Tool, profile.tools()));
    references.extend(resources(SharedResourceKind::Skill, profile.skills()));
    references.extend(resources(
      SharedResourceKind::Instruction,
      profile.instruction_resources(),
    ));
    references.extend(
      profile
        .secrets()
        .iter()
        .map(|name| Self::SecretName { name: name.clone() }),
    );
    references
  }

  /// The references of a team reviewer's body: a language reviewer's CLI kind, like a profile's; a decision reviewer
  /// names no CLI kind, no team resource and no secret, so it has none. The body is the source, as for a profile.
  pub fn of_reviewer(reviewer: &TeamReviewer) -> Vec<Self> {
    reviewer.cli().map(|cli| Self::CliKind { cli }).into_iter().collect()
  }

  /// The references of a team tool's body: the secret it sends or connects with, by name, as a profile's; a command
  /// tool names none. The body is the source, as for a profile.
  pub fn of_tool(tool: &TeamTool) -> Vec<Self> {
    tool
      .secret()
      .map(|name| Self::SecretName { name: name.clone() })
      .into_iter()
      .collect()
  }
}

/// One published version of a resource (`Version`), immutable and numbered by the server. `body` is the kind's body as
/// core stores it, `hash` core's content hash of it, `based_on` the version it was edited from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedVersion {
  pub resource: ResourceId,
  pub number: VersionNumber,
  pub body: serde_json::Value,
  pub hash: ContentHash,
  pub references: Vec<ReferenceTarget>,
  pub based_on: Option<VersionNumber>,
  pub published: Attribution,
  /// Computed by the server, never supplied by a publisher: core's `SharedBody` rule for the kind (team MCP servers,
  /// team agent profiles, team reviewers and tools) against the preceding version, or `true` for a body core cannot
  /// read; `false` for instructions, skills and pipelines (`docs/cloud-api.md`, "Which versions widen"). Absent on
  /// older servers; clients must still compare permissions locally before applying a version.
  #[serde(default)]
  pub widens: bool,
}

/// A version's content as an admin sends it: the body, and the references it names (`422 invalid { reason:
/// "references" }` when one does not hold).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionContent {
  pub body: serde_json::Value,
  pub references: Vec<ReferenceTarget>,
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
  pub references: Vec<ReferenceTarget>,
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
  pub references: Vec<ReferenceTarget>,
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
