//! The code index of a run's base revision (`orchester_index`, `orchester_core::code_index`) and ripgrep for the
//! agents that search it.
//!
//! - While the project has the code index on, the index is built, or reused, each time the run's task starts executing
//!   the run, before any step starts, and kept while the run's worktree exists. An agent that starts after the setting
//!   was turned on during the run builds it then.
//! - At each agent start the setting is read again. Off, the agent gets nothing. On, ripgrep is looked up on the
//!   agent's `PATH` and in the agent CLIs' installs and copied into `.orchester/tools/`, which goes first on the
//!   agent's `PATH`; the agent then gets the index folder and the internal `codebase-map` skill.
//! - An index that cannot be built or a ripgrep that is not found does not stop the run: the agent gets a runner
//!   problem in its activity saying why, and its invocation records what it got ([`CodeIndexUse`]).

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  code_index::{CodeIndexUnavailable, CodeIndexUse},
  command::Command,
  connection::{
    AgentConnection,
    stream::{AgentEvent, RunnerProblem},
  },
  id::RunId,
  revision::Revision,
  run::CodeIndexPrompt,
  store::{ConnectionStore, SettingsStore},
};
use orchester_exec::{ripgrep, search_path::search_path};
use orchester_index::{CodebaseIndexes, IndexError, Outcome};

use super::run::RunTask;

/// The code index of the run's base revision, as the run's task knows it.
#[derive(Clone, Debug, Default)]
pub(super) enum IndexState {
  /// Not built while this task executes the run: the project had it off.
  #[default]
  NotBuilt,
  Ready {
    folder: PathBuf,
  },
  Failed {
    detail: String,
  },
}

/// What an agent that starts now gets of the code index.
pub(super) struct AgentIndex {
  pub(super) used: CodeIndexUse,
  /// The index folder, when the agent gets it.
  pub(super) folder: Option<PathBuf>,
  /// The folder with the copy of ripgrep, which goes first on the agent's `PATH`.
  pub(super) tools: Option<PathBuf>,
  /// The runner problems to record in the agent's activity.
  pub(super) problems: Vec<AgentEvent>,
}

impl AgentIndex {
  const fn off() -> Self {
    Self {
      used: CodeIndexUse::Off,
      folder: None,
      tools: None,
      problems: Vec::new(),
    }
  }
}

/// Builds or reuses the index of `revision` for `run`, off the runtime's threads.
pub(super) async fn ensure(indexes: CodebaseIndexes, revision: Revision, run: RunId) -> IndexState {
  match tokio::task::spawn_blocking(move || indexes.ensure(&revision, run)).await {
    Ok(result) => state(run, result),
    Err(error) => {
      tracing::error!(%run, %error, "building the codebase index stopped");
      IndexState::Failed {
        detail: format!("building the index stopped: {error}"),
      }
    }
  }
}

fn state(run: RunId, result: Result<orchester_index::Ensured, IndexError>) -> IndexState {
  match result {
    Ok(ensured) => {
      match ensured.outcome {
        Outcome::Reused => tracing::info!(%run, path = %ensured.path.display(), "reusing the codebase index"),
        Outcome::Built(report) => tracing::info!(
          %run,
          path = %ensured.path.display(),
          mode = ?report.mode,
          files = report.files,
          read = report.read,
          millis = report.duration.as_millis(),
          "built the codebase index"
        ),
      }
      IndexState::Ready { folder: ensured.path }
    }
    Err(error) => {
      tracing::warn!(%run, %error, "the codebase index is unavailable");
      IndexState::Failed {
        detail: error.to_string(),
      }
    }
  }
}

impl RunTask {
  /// What an agent starting now gets of the code index, the index built first when the setting
  /// was turned on while the run executed.
  pub(super) fn agent_index(&mut self) -> Result<AgentIndex, String> {
    if !self.db.code_index().map_err(|error| error.to_string())?.is_on() {
      return Ok(AgentIndex::off());
    }
    let index = self.index_folder();
    let tools = self.provide_ripgrep()?;
    let (reason, problem) = match (index, &tools) {
      (Ok(folder), Some(_)) => {
        return Ok(AgentIndex {
          used: CodeIndexUse::Given {
            revision: self.run.base().clone(),
          },
          folder: Some(folder),
          tools,
          problems: Vec::new(),
        });
      }
      (Err(detail), _) => (
        CodeIndexUnavailable::BuildFailed { detail: detail.clone() },
        RunnerProblem::CodebaseIndexUnavailable { detail },
      ),
      (Ok(_), None) => (CodeIndexUnavailable::RipgrepNotFound, RunnerProblem::RipgrepNotFound),
    };
    Ok(AgentIndex {
      used: CodeIndexUse::Unavailable { reason },
      folder: None,
      tools,
      problems: vec![AgentEvent::RunnerProblem { problem }],
    })
  }

  /// The folder of the run's index, built now when the task has not built it; why there is none.
  fn index_folder(&mut self) -> Result<PathBuf, String> {
    if matches!(self.code_index, IndexState::NotBuilt) {
      let result = CodebaseIndexes::new(&self.root).ensure(self.run.base(), self.run.id());
      self.code_index = state(self.run.id(), result);
    }
    match &self.code_index {
      IndexState::Ready { folder } => Ok(folder.clone()),
      IndexState::Failed { detail } => Err(detail.clone()),
      IndexState::NotBuilt => Err("the index was not built".to_owned()),
    }
  }

  /// The folder holding the copy of ripgrep for an agent; `None` when none was found on the `PATH`
  /// the agent starts with (the project's, else the runner's) or with an agent CLI, or it could not
  /// be copied, which is logged.
  fn provide_ripgrep(&self) -> Result<Option<PathBuf>, String> {
    let environment = self.db.project_environment().map_err(|error| error.to_string())?;
    let path = environment.get("PATH").map(OsString::from).or_else(search_path);
    let executables: Vec<PathBuf> = self
      .db
      .connections()
      .map_err(|error| error.to_string())?
      .into_iter()
      // Connections of other kinds (GitHub) bundle no ripgrep.
      .filter_map(|connection| AgentConnection::try_from(connection).ok())
      .map(|connection| connection.executable().to_path_buf())
      .collect();
    let clis: Vec<&Path> = executables.iter().map(PathBuf::as_path).collect();
    let Some(found) = ripgrep::locate(path.as_deref(), &clis) else {
      return Ok(None);
    };
    match ripgrep::provide(&found, &self.root.join(".orchester").join("tools")) {
      Ok(folder) => Ok(Some(folder)),
      Err(error) => {
        tracing::warn!(%error, found = %found.path.display(), "ripgrep cannot be given to agents");
        Ok(None)
      }
    }
  }
}

/// `command` with `folder` first on its `PATH`.
pub(super) fn with_tools_on_path(command: Command, folder: &Path) -> Command {
  let current = command
    .env()
    .iter()
    .rev()
    .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
    .map(|(_, value)| OsString::from(value));
  let mut folders = vec![folder.to_path_buf()];
  folders.extend(current.iter().flat_map(std::env::split_paths));
  match std::env::join_paths(folders) {
    Ok(path) => command.with_env("PATH", &path.to_string_lossy()),
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "ripgrep's folder cannot go on the agent's PATH");
      command
    }
  }
}

/// What an agent's prompt says about the code index, from what it gets.
pub(super) enum IndexTold {
  Given {
    revision: String,
    folder: String,
    skill: String,
  },
  WithoutRipgrep,
  Nothing,
}

impl IndexTold {
  /// For an agent that gets `index`, with the `codebase-map` skill placed at `skill` when it gets
  /// the index.
  pub(super) fn new(index: &AgentIndex, skill: Option<&Path>) -> Self {
    match (&index.used, &index.folder, skill) {
      (CodeIndexUse::Given { revision }, Some(folder), Some(skill)) => Self::Given {
        revision: revision.to_string(),
        folder: folder.to_string_lossy().into_owned(),
        skill: skill.to_string_lossy().into_owned(),
      },
      (
        CodeIndexUse::Unavailable {
          reason: CodeIndexUnavailable::RipgrepNotFound,
        },
        ..,
      ) => Self::WithoutRipgrep,
      _ => Self::Nothing,
    }
  }

  pub(super) fn prompt(&self) -> Option<CodeIndexPrompt<'_>> {
    match self {
      Self::Given {
        revision,
        folder,
        skill,
      } => Some(CodeIndexPrompt::Given {
        revision,
        folder,
        skill,
      }),
      Self::WithoutRipgrep => Some(CodeIndexPrompt::WithoutRipgrep),
      Self::Nothing => None,
    }
  }
}
