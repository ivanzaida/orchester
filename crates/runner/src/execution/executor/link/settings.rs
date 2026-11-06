//! The settings area of the coordinator link: the project settings an agent's start, its tools and its worker read
//! (Project settings › Context, Approvals, Auto mode, Runs), as the project stores them now. Each is asked where and
//! when it was read before: at each agent start, at each request the worker routes, at each workspace borrowed and at
//! each try of a QA app's start, so a change applies from the next one as before. The Memory setting is the memory
//! area's ([`super::memory`]).

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  code_index::CodeIndexSetting,
  id::RunId,
  limits::QaSlots,
  permission::ApprovalMode,
  run::{EffectiveMode, review::AutoModeSettings},
  workspaces::WarmFolder,
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about the project's settings.
pub trait SettingsLink {
  /// Whether the project's agents get the code index.
  fn code_index(&self, request: &CodeIndexRequest) -> Result<CodeIndexSetting, LinkError>;

  /// Whether the project's agents may load the repository's own agent files.
  fn agent_files(&self, request: &AgentFilesRequest) -> Result<RepositoryAgentFiles, LinkError>;

  /// The mode the step of `request`'s run whose profile asks for its mode answers its agent's requests in now: by the
  /// project's *Allow Autopilot* and the run's switch to Manual.
  fn approval_mode(&self, request: &ApprovalModeRequest) -> Result<EffectiveMode, LinkError>;

  /// The reviewer a person chose for auto mode.
  fn auto_mode(&self, request: &AutoModeRequest) -> Result<AutoModeSettings, LinkError>;

  /// The project's warm folders, which a workspace's switch keeps or clears.
  fn warm_folders(&self, request: &WarmFoldersRequest) -> Result<WarmFolders, LinkError>;

  /// How many QA apps the project's runs may have open at once.
  fn qa_apps_at_once(&self, request: &QaAppsRequest) -> Result<QaAppsAtOnce, LinkError>;
}

/// Project settings › Context › code index: the project is the link's own, so the request names nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeIndexRequest {}

/// Project settings › Context › the repository's agent files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFilesRequest {}

/// The mode `run`'s step whose profile asks for `profile` answers its agent's requests in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalModeRequest {
  pub run: RunId,
  pub profile: ApprovalMode,
}

/// Project settings › Auto mode: the reviewer a person chose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoModeRequest {}

/// Project settings › Runs › warm folders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmFoldersRequest {}

/// The project's warm folders, in the order the person sees them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmFolders {
  pub folders: Vec<WarmFolder>,
}

/// Project settings › Runs › Limits › *QA apps at once*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QaAppsRequest {}

/// How many QA apps the project's runs may have open at once, as the project stores it; checked again on this side of
/// the link ([`QaAppsAtOnce::slots`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QaAppsAtOnce {
  pub count: u32,
}

impl QaAppsAtOnce {
  /// The answer for `slots`.
  pub fn of(slots: QaSlots) -> Self {
    Self {
      count: slots.get().get(),
    }
  }

  /// The slots the answer names; why not when it names a count outside the range a project can set.
  pub fn slots(self) -> Result<QaSlots, LinkError> {
    QaSlots::new(self.count.into()).map_err(|error| LinkError::other(&error))
  }
}

/// Whether the project's agents get the code index, asked through `link`.
pub fn code_index(link: &(impl SettingsLink + ?Sized)) -> Result<CodeIndexSetting, LinkError> {
  link.code_index(&CodeIndexRequest {})
}

/// Whether the project's agents may load the repository's own agent files, asked through `link`.
pub fn agent_files(link: &(impl SettingsLink + ?Sized)) -> Result<RepositoryAgentFiles, LinkError> {
  link.agent_files(&AgentFilesRequest {})
}

/// The reviewer a person chose for auto mode, asked through `link`.
pub fn auto_mode(link: &(impl SettingsLink + ?Sized)) -> Result<AutoModeSettings, LinkError> {
  link.auto_mode(&AutoModeRequest {})
}

/// The project's warm folders, asked through `link`.
pub fn warm_folders(link: &(impl SettingsLink + ?Sized)) -> Result<Vec<WarmFolder>, LinkError> {
  link.warm_folders(&WarmFoldersRequest {}).map(|answer| answer.folders)
}

/// How many QA apps the project's runs may have open at once, asked through `link`.
pub fn qa_slots(link: &(impl SettingsLink + ?Sized)) -> Result<QaSlots, LinkError> {
  link.qa_apps_at_once(&QaAppsRequest {})?.slots()
}
