//! Project settings › Code index (`i220W`, `h6yMk`, `NvR7K`): the setting, the indexes Orchester
//! keeps with the runs that use them, rebuilding one, and ripgrep for agents, looked up the way the
//! runner looks it up and installed only when a person asks. Read from the project's
//! `.orchester/index` and database and from the file system; blocks, so the UI calls it off the UI
//! thread.

mod install;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  code_index::CodeIndexSetting,
  command::{Command, CommandRunner},
  connection::AgentConnection,
  revision::Revision,
  store::{ConnectionStore, RunStore, SettingsStore, TaskStore},
};
use orchester_exec::{
  ProcessRunner,
  ripgrep::{
    self, LOCAL_APP_DATA, Source,
    install::{InstallPlan, InstallUnavailable},
  },
  search_path::search_path,
};
use orchester_index::{CodebaseIndexes, IndexError, IndexState};
use orchester_store::{ProjectDb, StoreError};

pub use self::install::{Install, cancel_install, ends_with_the_app, install_state, start_install};
use crate::messages::{self, Message};

/// `git rev-parse` answers at once; one that does not is stuck.
const GIT_TIMEOUT: Duration = Duration::from_secs(10);
/// `rg --version` answers at once; one that does not is stuck.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// A run whose worktree uses an index: the branch its checkout had, its task and its number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsedBy {
  pub branch: Option<Arc<str>>,
  pub task: Arc<str>,
  pub run: u32,
}

/// One revision of the index list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexRow {
  pub revision: Revision,
  pub used_by: Vec<UsedBy>,
  pub state: IndexState,
}

/// The index list as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexList(pub Arc<Vec<IndexRow>>);

impl IndexList {
  pub fn building(&self) -> bool {
    self.0.iter().any(|row| matches!(row.state, IndexState::Building(_)))
  }
}

pub fn setting_in(repo_root: &Path) -> Result<CodeIndexSetting, StoreError> {
  ProjectDb::open(repo_root)?.code_index()
}

pub fn save_setting_in(repo_root: &Path, setting: CodeIndexSetting) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_code_index(setting)
}

/// The indexes kept for the project at `repo_root`, with the runs that use them. A run's branch is
/// named only while it still points at the index's revision.
pub fn list_in(repo_root: &Path) -> Result<IndexList, Message> {
  let listed = CodebaseIndexes::new(repo_root).list().map_err(|error| {
    tracing::error!(%error, "cannot list the codebase indexes");
    Message::new("settings.codeIndex.listFailed")
  })?;
  let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let mut heads = BranchHeads::new(repo_root);
  let mut rows = Vec::with_capacity(listed.len());
  for listing in listed {
    let mut used_by = Vec::new();
    for id in listing.runs {
      // A run the database does not know (its record is unreadable) is left out of the text.
      let Ok(Some(run)) = db.run(id) else {
        continue;
      };
      let task = db
        .task(run.task())
        .map_err(|error| messages::store_error(&error))?
        .map_or_else(|| Arc::from(""), |task| Arc::from(task.key().to_string()));
      let branch = run
        .base_branch()
        .filter(|branch| heads.points_at(branch.as_str(), &listing.revision))
        .map(|branch| Arc::from(branch.as_str()));
      used_by.push(UsedBy {
        branch,
        task,
        run: run.number().get(),
      });
    }
    rows.push(IndexRow {
      revision: listing.revision,
      used_by,
      state: listing.state,
    });
  }
  Ok(IndexList(Arc::new(rows)))
}

/// The commits branches of a repository point at, each read once.
struct BranchHeads<'a> {
  repo_root: &'a Path,
  read: HashMap<String, Option<String>>,
}

impl<'a> BranchHeads<'a> {
  fn new(repo_root: &'a Path) -> Self {
    Self {
      repo_root,
      read: HashMap::new(),
    }
  }

  /// Whether `branch` points at `revision`; not when it is gone or cannot be read.
  fn points_at(&mut self, branch: &str, revision: &Revision) -> bool {
    let repo_root = self.repo_root;
    let head = self.read.entry(branch.to_owned()).or_insert_with(|| {
      let reference = format!("refs/heads/{branch}^{{commit}}");
      let command = Command::new(
        Path::new("git"),
        ["rev-parse", "--verify", "--quiet", reference.as_str()],
      )
      .with_current_dir(repo_root);
      match ProcessRunner::new(GIT_TIMEOUT).run(&command) {
        Ok(output) if output.code == Some(0) => Some(output.stdout.trim().to_owned()),
        Ok(_) => None,
        Err(error) => {
          tracing::debug!(%error, branch, "cannot read where a branch points");
          None
        }
      }
    });
    head.as_deref() == Some(revision.as_str())
  }
}

/// Builds the index of `revision` again (*Rebuild*, *Retry*); why it failed.
pub fn rebuild_in(repo_root: &Path, revision: &Revision) -> Result<(), Message> {
  CodebaseIndexes::new(repo_root)
    .rebuild(revision)
    .map(drop)
    .map_err(|error: IndexError| {
      tracing::warn!(%error, %revision, "the codebase index could not be rebuilt");
      match error {
        IndexError::InUse { .. } => Message::new("settings.codeIndex.rebuildInUse"),
        error => Message::new("settings.codeIndex.rebuildFailed").with("reason", error.to_string()),
      }
    })
}

/// Whether agents have ripgrep, and how it could be installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RipgrepCheck {
  pub tool: SearchTool,
  pub install: Result<InstallPlan, InstallUnavailable>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchTool {
  /// Found where the runner finds it for agents; `version` is what `rg --version` said.
  Found {
    version: Option<String>,
    source: Source,
  },
  NotFound,
}

/// Looks for ripgrep as the runner does for an agent of the project at `repo_root` (the project's
/// `PATH`, else Orchester's, then where winget installs, then the agent CLIs' installs). Only
/// looks: the runner copies it into the tools folder when an agent starts, and removes old copies.
pub fn check_ripgrep(repo_root: &Path) -> Result<RipgrepCheck, Message> {
  let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let environment = db
    .project_environment()
    .map_err(|error| messages::store_error(&error))?;
  let path = environment
    .get("PATH")
    .map(std::ffi::OsString::from)
    .or_else(search_path);
  let local_app_data = environment
    .get(LOCAL_APP_DATA)
    .map(std::ffi::OsString::from)
    .or_else(|| std::env::var_os(LOCAL_APP_DATA));
  let executables: Vec<PathBuf> = db
    .connections()
    .map_err(|error| messages::store_error(&error))?
    .into_iter()
    // Connections of other kinds (GitHub) and native agents, which run no program, bundle no ripgrep.
    .filter_map(|connection| AgentConnection::try_from(connection).ok())
    .filter_map(|connection| connection.executable().map(Path::to_path_buf))
    .collect();
  let clis: Vec<&Path> = executables.iter().map(PathBuf::as_path).collect();
  let tool = match ripgrep::locate(path.as_deref(), local_app_data.as_deref(), &clis) {
    None => SearchTool::NotFound,
    Some(found) => SearchTool::Found {
      version: version(&found.path),
      source: found.source,
    },
  };
  let install = InstallPlan::find(search_path().as_deref());
  Ok(RipgrepCheck { tool, install })
}

/// `ripgrep 14.1.1` from the first line `rg --version` prints; `None` when it says nothing usable.
fn version(program: &Path) -> Option<String> {
  // A version that cannot be read is not shown; ripgrep was found all the same.
  let output = ProcessRunner::new(VERSION_TIMEOUT)
    .run(&Command::new(program, ["--version"]))
    .ok()?;
  let first = output.stdout.lines().next()?;
  let mut words = first.split_whitespace();
  match (words.next(), words.next()) {
    (Some(name), Some(number)) => Some(format!("{name} {number}")),
    _ => None,
  }
}
