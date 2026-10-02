//! The code index of a run's base revision (`orchester_index`, `orchester_core::code_index`) and ripgrep for the
//! agents that search it.
//!
//! - While the project has the code index on, the index is built, or reused, each time the run's task starts executing
//!   the run, and kept while the run's worktree exists. The build runs in the background: agents' news, requests and
//!   time limits are handled meanwhile, and no agent starts until it ends. When the setting was turned on while the run
//!   executed, the next agent start begins the build in the background; agents that start before it ends get no index
//!   ([`CodeIndexUnavailable::Building`]), later ones get it.
//! - At each agent start the setting is read again. Off, the agent gets nothing. On, ripgrep is looked up on the
//!   agent's `PATH` and in the agent CLIs' installs and copied into `.orchester/tools/`, which goes first on the
//!   agent's `PATH`; the agent then gets the index folder and the internal `orchester-codebase-index` skill.
//! - An index that cannot be built or a ripgrep that is not found does not stop the run: the agent gets a runner
//!   problem in its activity saying why, and its invocation records what it got ([`CodeIndexUse`]).

use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  code_index::{CodeIndexUnavailable, CodeIndexUse, RipgrepFallback, RipgrepMissing},
  command::Command,
  connection::{
    AgentConnection, AgentProvider,
    stream::{AgentEvent, RunnerProblem},
  },
  id::RunId,
  revision::Revision,
  run::CodeIndexPrompt,
  store::{ConnectionStore, SettingsStore},
};
use orchester_exec::{
  ripgrep::{self, LOCAL_APP_DATA},
  search_path::search_path,
};
use orchester_index::{CodebaseIndexes, IndexError, Outcome};
use orchester_store::StoreError;

use super::run::RunTask;

/// The code index of the run's base revision, as the run's task knows it.
#[derive(Clone, Debug)]
pub(super) enum IndexState {
  /// Not built while this task executes the run: the project had it off when the task started, or
  /// it is being built ([`IndexBuild`]).
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
  /// Why the index could not be built, when it could not.
  build_failed: Option<String>,
  /// Why the agent has no ripgrep, when it has none.
  ripgrep: Option<RipgrepMissing>,
}

impl AgentIndex {
  const fn off() -> Self {
    Self {
      used: CodeIndexUse::Off,
      folder: None,
      tools: None,
      build_failed: None,
      ripgrep: None,
    }
  }

  /// The runner problems to record in the activity of an agent on `provider`'s CLI: an index that
  /// could not be built, and a missing ripgrep with how the agent searches instead.
  pub(super) fn problems(&self, provider: AgentProvider) -> Vec<AgentEvent> {
    let fallback = RipgrepFallback::of(provider, self.folder.is_some());
    let index = self
      .build_failed
      .clone()
      .map(|detail| RunnerProblem::CodebaseIndexUnavailable { detail });
    let ripgrep = self.ripgrep.as_ref().map(|missing| match missing {
      RipgrepMissing::NotFound => RunnerProblem::RipgrepNotFound { fallback },
      RipgrepMissing::Unavailable { detail } => RunnerProblem::RipgrepUnavailable {
        detail: detail.clone(),
        fallback,
      },
    });
    index
      .into_iter()
      .chain(ripgrep)
      .map(|problem| AgentEvent::RunnerProblem { problem })
      .collect()
  }
}

/// A build of the run's index in the background.
pub(super) struct IndexBuild {
  handle: tokio::task::JoinHandle<IndexState>,
  /// Begun when the task started: no agent starts until it ends.
  holds_starts: bool,
}

/// The index the build in `build` came to, once it ended; never while there is none.
pub(super) async fn built(build: &mut Option<IndexBuild>) -> IndexState {
  let Some(build) = build else {
    return std::future::pending().await;
  };
  match (&mut build.handle).await {
    Ok(state) => state,
    Err(error) => IndexState::Failed {
      detail: format!("building the index stopped: {error}"),
    },
  }
}

/// Builds or reuses the index of `revision` for `run`, off the runtime's threads.
async fn ensure(indexes: CodebaseIndexes, revision: Revision, run: RunId) -> IndexState {
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
  /// Begins building the index in the background when the project has the code index on; no agent
  /// starts until the build ends.
  pub(super) fn begin_code_index(&mut self) -> Result<(), StoreError> {
    if self.db.code_index()?.is_on() {
      self.build_code_index(true);
    }
    Ok(())
  }

  fn build_code_index(&mut self, holds_starts: bool) {
    let indexes = CodebaseIndexes::new(&self.root);
    let handle = tokio::spawn(ensure(indexes, self.run.base().clone(), self.run.id()));
    self.index_build = Some(IndexBuild { handle, holds_starts });
  }

  /// Agents wait for the build begun when the task started.
  pub(super) fn index_holds_starts(&self) -> bool {
    self.index_build.as_ref().is_some_and(|build| build.holds_starts)
  }

  /// Takes in the index the background build came to.
  pub(super) fn code_index_built(&mut self, state: IndexState) {
    self.index_build = None;
    self.code_index = state;
  }

  /// What an agent starting now gets of the code index. When the setting was turned on while the
  /// run executed, the build begins now and the agent gets no index.
  pub(super) fn agent_index(&mut self) -> Result<AgentIndex, String> {
    if !self.db.code_index().map_err(|error| error.to_string())?.is_on() {
      return Ok(AgentIndex::off());
    }
    let index = self.index_folder();
    let (tools, ripgrep) = match self.provide_ripgrep()? {
      Ok(folder) => (Some(folder), None),
      Err(missing) => (None, Some(missing)),
    };
    let (used, folder, build_failed) = match index {
      Ok(folder) => (
        CodeIndexUse::Given {
          revision: self.run.base().clone(),
          ripgrep: ripgrep.clone(),
        },
        Some(folder),
        None,
      ),
      Err(reason) => {
        let build_failed = match &reason {
          CodeIndexUnavailable::BuildFailed { detail } => Some(detail.clone()),
          CodeIndexUnavailable::Building => None,
        };
        (CodeIndexUse::Unavailable { reason }, None, build_failed)
      }
    };
    Ok(AgentIndex {
      used,
      folder,
      tools,
      build_failed,
      ripgrep,
    })
  }

  /// The folder of the run's index; why there is none. A failed build may have been retried from
  /// Project settings since: an index that is there now is used.
  fn index_folder(&mut self) -> Result<PathBuf, CodeIndexUnavailable> {
    if self.index_build.is_some() {
      return Err(CodeIndexUnavailable::Building);
    }
    if let IndexState::Failed { .. } | IndexState::NotBuilt = self.code_index
      && let Some(folder) = CodebaseIndexes::new(&self.root).existing(self.run.base())
    {
      self.code_index = IndexState::Ready { folder };
    }
    match &self.code_index {
      IndexState::Ready { folder } => Ok(folder.clone()),
      IndexState::Failed { detail } => Err(CodeIndexUnavailable::BuildFailed { detail: detail.clone() }),
      // The setting was turned on after the task started.
      IndexState::NotBuilt => {
        self.build_code_index(false);
        Err(CodeIndexUnavailable::Building)
      }
    }
  }

  /// The folder holding the copy of ripgrep for an agent; else why the agent has none: none was
  /// found on the `PATH` the agent starts with (the project's, else the runner's), where winget
  /// installs for the agent's user or with an agent CLI, or the one found could not be copied.
  fn provide_ripgrep(&self) -> Result<Result<PathBuf, RipgrepMissing>, String> {
    let environment = self.db.project_environment().map_err(|error| error.to_string())?;
    let path = environment.get("PATH").map(OsString::from).or_else(search_path);
    let local_app_data = environment
      .get(LOCAL_APP_DATA)
      .map(OsString::from)
      .or_else(|| std::env::var_os(LOCAL_APP_DATA));
    let executables: Vec<PathBuf> = self
      .db
      .connections()
      .map_err(|error| error.to_string())?
      .into_iter()
      // Connections of other kinds (GitHub) and native agents, which run no program, bundle no ripgrep.
      .filter_map(|connection| AgentConnection::try_from(connection).ok())
      .filter_map(|connection| connection.executable().map(Path::to_path_buf))
      .collect();
    let clis: Vec<&Path> = executables.iter().map(PathBuf::as_path).collect();
    let Some(found) = ripgrep::locate(path.as_deref(), local_app_data.as_deref(), &clis) else {
      return Ok(Err(RipgrepMissing::NotFound));
    };
    Ok(
      ripgrep::provide(&found, &self.root.join(".orchester").join("tools")).map_err(|error| {
        tracing::warn!(%error, found = %found.path.display(), "ripgrep cannot be given to agents");
        RipgrepMissing::Unavailable {
          detail: error.to_string(),
        }
      }),
    )
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
    git_grep: bool,
    /// The index's `map.md`; empty when it cannot be read.
    map: String,
  },
  WithoutRipgrep,
  Nothing,
}

impl IndexTold {
  /// For an agent that gets `index`, with the `orchester-codebase-index` skill placed at `skill` when it gets
  /// the index.
  /// Claude Code (`provider`) has a search tool of its own: it is not steered to `git grep`.
  pub(super) fn new(index: &AgentIndex, skill: Option<&Path>, provider: AgentProvider) -> Self {
    let codex_without_ripgrep = index.tools.is_none() && provider == AgentProvider::CodexCli;
    match (&index.used, &index.folder, skill) {
      (CodeIndexUse::Given { revision, .. }, Some(folder), Some(skill)) => Self::Given {
        revision: revision.to_string(),
        folder: folder.to_string_lossy().into_owned(),
        skill: skill.to_string_lossy().into_owned(),
        git_grep: codex_without_ripgrep,
        map: read_map(folder),
      },
      (CodeIndexUse::Unavailable { .. }, ..) if codex_without_ripgrep => Self::WithoutRipgrep,
      _ => Self::Nothing,
    }
  }

  pub(super) fn prompt(&self) -> Option<CodeIndexPrompt<'_>> {
    match self {
      Self::Given {
        revision,
        folder,
        skill,
        git_grep,
        map,
      } => Some(CodeIndexPrompt::Given {
        revision,
        folder,
        skill,
        git_grep: *git_grep,
        map,
      }),
      Self::WithoutRipgrep => Some(CodeIndexPrompt::WithoutRipgrep),
      Self::Nothing => None,
    }
  }
}

/// The text of the `map.md` of the index in `folder`. A map that cannot be read leaves only its
/// excerpt out of the prompt: the agent still gets the index and reads the map itself.
fn read_map(folder: &Path) -> String {
  let file = folder.join("map.md");
  fs::read_to_string(&file).unwrap_or_else(|error| {
    tracing::warn!(%error, file = %file.display(), "the index map cannot be read; the prompt goes without its excerpt");
    String::new()
  })
}
