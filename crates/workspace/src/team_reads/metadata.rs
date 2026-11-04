//! A team workspace's metadata as the server reads it (S10): the project with its policy and the caller's
//! capabilities, and the capabilities alone. Both belong to [`QueryScope::Workspace`]. Capabilities are advisory: every
//! command is checked again on the server.

use orchester_api::v2::{Capabilities, QueryScope, WorkspaceMetadata};

use super::{TeamProject, TeamRead, TeamReadError};

impl TeamProject {
  pub async fn metadata(&self) -> Result<TeamRead<WorkspaceMetadata>, TeamReadError> {
    self
      .read(QueryScope::Workspace {}, |inner| async move {
        inner.session.authority().workspace().await
      })
      .await
  }

  pub async fn capabilities(&self) -> Result<TeamRead<Capabilities>, TeamReadError> {
    self
      .read(QueryScope::Workspace {}, |inner| async move {
        inner.session.authority().capabilities().await
      })
      .await
  }
}
