//! The resources area of the local link: resources, pinned versions and blobs read from the project database.

use std::ops::Deref;

use orchester_core::{
  resource::{AgentProfile, Instruction, ResourceBody, Skill, Tool},
  store::{BlobStore, ResourceStore},
};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  resources::{
    Blob, BlobRequest, PinnedBody, PinnedKind, ResourceLink, ResourceNames, ResourceRequest, VersionRequest,
  },
};

impl<D: Deref<Target = ProjectDb>> ResourceLink for LocalLink<D> {
  fn resource(&self, request: &ResourceRequest) -> Result<Option<ResourceNames>, LinkError> {
    let resource = self.0.resource(request.resource).map_err(|error| read_failed(&error))?;
    Ok(resource.map(|resource| ResourceNames {
      name: resource.name().as_str().to_owned(),
      slug: resource.slug().as_str().to_owned(),
    }))
  }

  fn version(&self, request: &VersionRequest) -> Result<Option<PinnedBody>, LinkError> {
    match request.kind {
      PinnedKind::AgentProfile => self.body::<AgentProfile>(request, PinnedBody::AgentProfile),
      PinnedKind::Instruction => self.body::<Instruction>(request, PinnedBody::Instruction),
      PinnedKind::Skill => self.body::<Skill>(request, PinnedBody::Skill),
      PinnedKind::Tool => self.body::<Tool>(request, PinnedBody::Tool),
    }
  }

  fn blob(&self, request: &BlobRequest) -> Result<Blob, LinkError> {
    let content = self.0.blob(&request.hash).map_err(|error| read_failed(&error))?;
    Ok(Blob { content })
  }
}

impl<D: Deref<Target = ProjectDb>> LocalLink<D> {
  /// The body of the version `request` names, read as a `B` and answered as `answer` makes it.
  fn body<B: ResourceBody + Clone>(
    &self,
    request: &VersionRequest,
    answer: fn(B) -> PinnedBody,
  ) -> Result<Option<PinnedBody>, LinkError> {
    let version = self
      .0
      .version::<B>(request.resource, request.number)
      .map_err(|error| read_failed(&error))?;
    Ok(version.map(|version| answer(version.body().clone())))
  }
}
