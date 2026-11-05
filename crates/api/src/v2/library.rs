//! The shared configuration and the workspace itself as S10 reads: the resource library and a resource's versions in
//! pages of the existing public resource types, the pickers a form chooses a resource with, and the workspace's
//! metadata. A request leaves out what it does not narrow by; an unknown field is refused.

use orchester_core::{id::ResourceId, resource::VersionNumber, sharing::ClaimStalenessThreshold};
use serde::{Deserialize, Serialize};

use super::{Capabilities, EventCursor, Page, PageRequest, SearchText, SharedProject, WorkspacePolicy};
use crate::{SharedResource, SharedResourceKind, SharedVersion};

/// `{kind?, archived, search?, page}`: one page of the library, of one kind when `kind` names one, archived resources
/// only with `archived`, and those whose name or slug holds `search`, ignoring case.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLibraryRequest {
  #[serde(default)]
  pub kind: Option<SharedResourceKind>,
  #[serde(default)]
  pub archived: bool,
  #[serde(default)]
  pub search: Option<SearchText>,
  #[serde(default)]
  pub page: PageRequest,
}

/// The library's resources, in creation order.
pub type ResourceLibraryPage = Page<SharedResource>;

/// A resource's published versions, oldest first; read with a [`PageRequest`].
pub type ResourceVersionsPage = Page<SharedVersion>;

/// `{kind, search?, page}`: the resources of `kind` a form may choose, those whose name or slug holds `search`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePickerRequest {
  pub kind: SharedResourceKind,
  #[serde(default)]
  pub search: Option<SearchText>,
  #[serde(default)]
  pub page: PageRequest,
}

/// A resource a picker offers: one that is not archived and has a published version, `latest` being its newest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceChoice {
  pub id: ResourceId,
  pub kind: SharedResourceKind,
  pub slug: String,
  pub name: String,
  pub latest: VersionNumber,
}

/// A picker's resources, in creation order.
pub type ResourcePickerPage = Page<ResourceChoice>;

/// The workspace as its pages open on it: the project, its effective policy, the server's claim staleness threshold
/// and the caller's capabilities, at snapshot `cursor`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceMetadata {
  pub project: SharedProject,
  pub policy: WorkspacePolicy,
  pub capabilities: Capabilities,
  pub cursor: EventCursor,
  /// After how long without a heartbeat a claim goes stale, in whole seconds. Every reader of the workspace reads
  /// it; only a server admin changes it (`ServerSettings`), so it is the server's, not the workspace policy's.
  pub claim_staleness_seconds: ClaimStalenessThreshold,
}
