//! The settings area of the local link: the project settings, read from the project database at each ask.

use std::ops::Deref;

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  code_index::CodeIndexSetting,
  run::{EffectiveMode, review::AutoModeSettings},
  store::{SettingsStore, WorkspaceStore},
};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  settings::{
    AgentFilesRequest, ApprovalModeRequest, AutoModeRequest, CodeIndexRequest, QaAppsAtOnce, QaAppsRequest,
    SettingsLink, WarmFolders, WarmFoldersRequest,
  },
};

impl<D: Deref<Target = ProjectDb>> SettingsLink for LocalLink<D> {
  fn code_index(&self, _: &CodeIndexRequest) -> Result<CodeIndexSetting, LinkError> {
    self.0.code_index().map_err(|error| read_failed(&error))
  }

  fn agent_files(&self, _: &AgentFilesRequest) -> Result<RepositoryAgentFiles, LinkError> {
    self.0.repository_agent_files().map_err(|error| read_failed(&error))
  }

  fn approval_mode(&self, request: &ApprovalModeRequest) -> Result<EffectiveMode, LinkError> {
    self
      .0
      .approval_mode(request.run, request.profile)
      .map_err(|error| read_failed(&error))
  }

  fn auto_mode(&self, _: &AutoModeRequest) -> Result<AutoModeSettings, LinkError> {
    self.0.auto_mode_settings().map_err(|error| read_failed(&error))
  }

  fn warm_folders(&self, _: &WarmFoldersRequest) -> Result<WarmFolders, LinkError> {
    let settings = self.0.workspace_settings().map_err(|error| read_failed(&error))?;
    Ok(WarmFolders {
      folders: settings.folders,
    })
  }

  fn qa_apps_at_once(&self, _: &QaAppsRequest) -> Result<QaAppsAtOnce, LinkError> {
    let limits = self.0.project_limits().map_err(|error| read_failed(&error))?;
    Ok(QaAppsAtOnce::of(limits.qa_slots()))
  }
}
