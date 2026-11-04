//! The resources area of the local link: resources, pinned versions and blobs read from the project database. In the
//! runner's own tests each request and answer crosses as JSON (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::{
  resource::{AgentProfile, Instruction, ResourceBody, Skill, Tool},
  store::{BlobStore, ResourceStore},
};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
use crate::execution::executor::link::{
  LinkError,
  resources::{
    Blob, BlobRequest, PinnedBody, PinnedKind, ResourceLink, ResourceNames, ResourceRequest, VersionRequest,
  },
};

impl<D: Deref<Target = ProjectDb>> ResourceLink for LocalLink<D> {
  fn resource(&self, request: &ResourceRequest) -> Result<Option<ResourceNames>, LinkError> {
    let answer = self
      .0
      .resource(sent_request(request).resource)
      .map(|resource| {
        resource.map(|resource| ResourceNames {
          name: resource.name().as_str().to_owned(),
          slug: resource.slug().as_str().to_owned(),
        })
      })
      .map_err(|error| read_failed(&error));
    sent_answer(answer)
  }

  fn version(&self, request: &VersionRequest) -> Result<Option<PinnedBody>, LinkError> {
    let request = sent_request(request);
    let answer = match request.kind {
      PinnedKind::AgentProfile => self.body::<AgentProfile>(request, PinnedBody::AgentProfile),
      PinnedKind::Instruction => self.body::<Instruction>(request, PinnedBody::Instruction),
      PinnedKind::Skill => self.body::<Skill>(request, PinnedBody::Skill),
      PinnedKind::Tool => self.body::<Tool>(request, PinnedBody::Tool),
    };
    sent_answer(answer)
  }

  fn blob(&self, request: &BlobRequest) -> Result<Blob, LinkError> {
    let answer = self
      .0
      .blob(&sent_request(request).hash)
      .map(|content| Blob { content })
      .map_err(|error| read_failed(&error));
    sent_answer(answer)
  }
}

/// `request` as the coordinator receives it.
fn sent_request<T>(request: &T) -> &T
where
  T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
  #[cfg(feature = "test-agent")]
  sent::survives_sending(request);
  request
}

/// `answer` as the executor receives it.
fn sent_answer<T>(answer: T) -> T
where
  T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
  #[cfg(feature = "test-agent")]
  sent::survives_sending(&answer);
  answer
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
