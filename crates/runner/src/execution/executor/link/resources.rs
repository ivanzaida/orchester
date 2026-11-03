//! The resources area of the coordinator link: the published versions a run pinned, read by id when a step needs
//! them (agent profiles, instructions, skills, tools), the names of their resources, removed or not, and the project
//! blobs they refer to (a skill's reference files).

use orchester_core::{
  blob::ContentHash,
  id::ResourceId,
  resource::{AgentProfile, Instruction, Skill, Slug, Tool, VersionNumber},
  run::PinnedVersion,
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about the project's resources.
pub trait ResourceLink {
  /// The names of the resource `request` names, removed or not: a run that pinned it still names it. `None` when no
  /// such resource exists.
  fn resource(&self, request: &ResourceRequest) -> Result<Option<ResourceNames>, LinkError>;

  /// The body of the published version `request` names; `None` when the version is missing.
  fn version(&self, request: &VersionRequest) -> Result<Option<PinnedBody>, LinkError>;

  /// The content of the blob `request` names, checked against its hash.
  fn blob(&self, request: &BlobRequest) -> Result<Blob, LinkError>;
}

/// A resource, read by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRequest {
  pub resource: ResourceId,
}

/// What prompts and the steps' records call a resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNames {
  pub name: String,
  pub slug: String,
}

impl ResourceNames {
  /// The slug, checked again on this side of the link.
  pub fn slug(&self) -> Result<Slug, LinkError> {
    Slug::new(&self.slug).map_err(|error| LinkError::other(&error))
  }
}

/// The kinds of resource whose pinned versions the executor reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinnedKind {
  AgentProfile,
  Instruction,
  Skill,
  Tool,
}

/// A published version of a resource of `kind`, by its resource and number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRequest {
  pub kind: PinnedKind,
  pub resource: ResourceId,
  pub number: VersionNumber,
}

/// The body of a published version, of the kind it was asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinnedBody {
  AgentProfile(AgentProfile),
  Instruction(Instruction),
  Skill(Skill),
  Tool(Tool),
}

/// A project blob, by its content hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRequest {
  pub hash: ContentHash,
}

/// A blob's content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blob {
  pub content: Vec<u8>,
}

/// A resource body the executor reads as a pinned version.
pub trait Pinned: Sized {
  const KIND: PinnedKind;

  /// The body `answer` holds, when it is of this kind.
  fn from_body(answer: PinnedBody) -> Option<Self>;
}

macro_rules! pinned {
  ($($body:ident),*) => {$(
    impl Pinned for $body {
      const KIND: PinnedKind = PinnedKind::$body;

      fn from_body(answer: PinnedBody) -> Option<Self> {
        match answer {
          PinnedBody::$body(body) => Some(body),
          _ => None,
        }
      }
    }
  )*};
}

pinned!(AgentProfile, Instruction, Skill, Tool);

/// The body of version `pin` of a `B`, asked through `link`; `None` when the version is missing.
pub fn pinned_version<B: Pinned>(
  link: &(impl ResourceLink + ?Sized),
  pin: &PinnedVersion,
) -> Result<Option<B>, LinkError> {
  let request = VersionRequest {
    kind: B::KIND,
    resource: pin.resource,
    number: pin.number,
  };
  let Some(answer) = link.version(&request)? else {
    return Ok(None);
  };
  B::from_body(answer).map(Some).ok_or_else(|| LinkError {
    detail: format!("the link answered another kind of resource than {:?}", B::KIND),
    failure: orchester_core::run::RunnerFailure::Other,
  })
}

/// The names of `resource`, asked through `link`; `None` when it does not exist.
pub fn resource_names(
  link: &(impl ResourceLink + ?Sized),
  resource: ResourceId,
) -> Result<Option<ResourceNames>, LinkError> {
  link.resource(&ResourceRequest { resource })
}

/// The content of blob `hash`, asked through `link`.
pub fn blob(link: &(impl ResourceLink + ?Sized), hash: &ContentHash) -> Result<Vec<u8>, LinkError> {
  link.blob(&BlobRequest { hash: hash.clone() }).map(|blob| blob.content)
}
