//! A team project's shared configuration as the server reads it (S10): a page of the library, a page of a picker's
//! choices, a page of a resource's published versions and one version with its body. Only what is asked for is read;
//! no resource's whole history is. The library and the pickers belong to [`QueryScope::Library`], a resource's
//! versions to its [`QueryScope::Resource`].

use orchester_api::{
  SharedVersion,
  v2::{
    PageRequest, QueryScope, ResourceLibraryPage, ResourceLibraryRequest, ResourcePickerPage, ResourcePickerRequest,
    ResourceVersionsPage,
  },
};
use orchester_core::{id::ResourceId, resource::VersionNumber};

use super::{TeamProject, TeamRead, TeamReadError};

impl TeamProject {
  pub async fn library_page(
    &self,
    request: ResourceLibraryRequest,
  ) -> Result<TeamRead<ResourceLibraryPage>, TeamReadError> {
    self
      .read(QueryScope::Library {}, |inner| {
        let request = request.clone();
        async move { inner.session.authority().library(&request).await }
      })
      .await
  }

  /// One page of the resources a form may choose: not archived, with a published version.
  pub async fn picker_page(
    &self,
    request: ResourcePickerRequest,
  ) -> Result<TeamRead<ResourcePickerPage>, TeamReadError> {
    self
      .read(QueryScope::Library {}, |inner| {
        let request = request.clone();
        async move { inner.session.authority().picker(&request).await }
      })
      .await
  }

  /// One page of `resource`'s published versions, oldest first.
  pub async fn version_page(
    &self,
    resource: ResourceId,
    page: PageRequest,
  ) -> Result<TeamRead<ResourceVersionsPage>, TeamReadError> {
    self
      .read(QueryScope::Resource { resource }, |inner| {
        let page = page.clone();
        async move { inner.session.authority().versions(resource, &page).await }
      })
      .await
  }

  pub async fn version(
    &self,
    resource: ResourceId,
    version: VersionNumber,
  ) -> Result<TeamRead<SharedVersion>, TeamReadError> {
    self
      .read(QueryScope::Resource { resource }, |inner| async move {
        inner.session.authority().version(resource, version).await
      })
      .await
  }
}
