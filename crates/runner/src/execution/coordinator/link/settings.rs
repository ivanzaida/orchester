//! The settings area of the local link: the project settings, read from the project database at each ask. In the
//! runner's own tests each request and answer crosses as JSON (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  code_index::CodeIndexSetting,
  run::{EffectiveMode, review::AutoModeSettings},
  store::{SettingsStore, WorkspaceStore},
};
use orchester_store::ProjectDb;

use super::{LocalLink, answered, read_failed};
use crate::execution::executor::link::{
  LinkError,
  settings::{
    AgentFilesRequest, ApprovalModeRequest, AutoModeRequest, CodeIndexRequest, QaAppsAtOnce, QaAppsRequest,
    SettingsLink, WarmFolders, WarmFoldersRequest,
  },
};

impl<D: Deref<Target = ProjectDb>> SettingsLink for LocalLink<D> {
  fn code_index(&self, request: &CodeIndexRequest) -> Result<CodeIndexSetting, LinkError> {
    answered(request, || self.0.code_index().map_err(|error| read_failed(&error)))
  }

  fn agent_files(&self, request: &AgentFilesRequest) -> Result<RepositoryAgentFiles, LinkError> {
    answered(request, || {
      self.0.repository_agent_files().map_err(|error| read_failed(&error))
    })
  }

  fn approval_mode(&self, request: &ApprovalModeRequest) -> Result<EffectiveMode, LinkError> {
    answered(request, || {
      self
        .0
        .approval_mode(request.run, request.profile)
        .map_err(|error| read_failed(&error))
    })
  }

  fn auto_mode(&self, request: &AutoModeRequest) -> Result<AutoModeSettings, LinkError> {
    answered(request, || {
      self.0.auto_mode_settings().map_err(|error| read_failed(&error))
    })
  }

  fn warm_folders(&self, request: &WarmFoldersRequest) -> Result<WarmFolders, LinkError> {
    answered(request, || {
      let settings = self.0.workspace_settings().map_err(|error| read_failed(&error))?;
      Ok(WarmFolders {
        folders: settings.folders,
      })
    })
  }

  fn qa_apps_at_once(&self, request: &QaAppsRequest) -> Result<QaAppsAtOnce, LinkError> {
    answered(request, || {
      let limits = self.0.project_limits().map_err(|error| read_failed(&error))?;
      Ok(QaAppsAtOnce::of(limits.qa_slots()))
    })
  }
}
