//! The answer: each resource and each pipeline with how it stands, and the hub's groups.

use orchester_core::{
  id::ResourceId,
  resource::{ResourceKind, VersionNumber},
};

use super::standing::{Group, ReadinessStanding};

/// One profile, MCP server or tool, as it stands on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceReadiness {
  pub(super) resource: ResourceId,
  pub(super) kind: ResourceKind,
  pub(super) name: String,
  pub(super) number: VersionNumber,
  pub(super) standing: ReadinessStanding,
  /// A team MCP server that is not switched on here.
  pub(super) off_here: bool,
}

impl ResourceReadiness {
  pub const fn resource(&self) -> ResourceId {
    self.resource
  }

  pub const fn kind(&self) -> ResourceKind {
    self.kind
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  /// The version this computer uses, which is the one the answer is about.
  pub const fn number(&self) -> VersionNumber {
    self.number
  }

  pub const fn standing(&self) -> &ReadinessStanding {
    &self.standing
  }

  pub fn group(&self) -> Group {
    if self.off_here {
      Group::OffHere
    } else if self.standing.is_ready() {
      Group::Ready
    } else {
      Group::NeedsYouHere
    }
  }
}

/// One pipeline: ready when every profile and tool its steps run, and its integrator, is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineReadiness {
  pub(super) pipeline: ResourceId,
  pub(super) name: String,
  pub(super) number: VersionNumber,
  pub(super) standing: ReadinessStanding,
}

impl PipelineReadiness {
  pub const fn pipeline(&self) -> ResourceId {
    self.pipeline
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub const fn number(&self) -> VersionNumber {
    self.number
  }

  pub const fn standing(&self) -> &ReadinessStanding {
    &self.standing
  }
}

/// Every profile, MCP server and tool, then every pipeline, the project offers, each with a version this computer uses;
/// a resource with no version is not listed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectReadiness {
  pub(super) resources: Vec<ResourceReadiness>,
  pub(super) pipelines: Vec<PipelineReadiness>,
}

impl ProjectReadiness {
  /// Profiles, then MCP servers, then tools, each by slug.
  pub fn resources(&self) -> &[ResourceReadiness] {
    &self.resources
  }

  pub fn pipelines(&self) -> &[PipelineReadiness] {
    &self.pipelines
  }

  pub fn resource(&self, resource: ResourceId) -> Option<&ResourceReadiness> {
    self.resources.iter().find(|found| found.resource == resource)
  }

  pub fn pipeline(&self, pipeline: ResourceId) -> Option<&PipelineReadiness> {
    self.pipelines.iter().find(|found| found.pipeline == pipeline)
  }

  /// The resources of `group`, in the order of [`Self::resources`].
  pub fn group(&self, group: Group) -> impl Iterator<Item = &ResourceReadiness> {
    self.resources.iter().filter(move |found| found.group() == group)
  }
}
