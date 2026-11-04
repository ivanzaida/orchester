//! The project's runner, as the chat's git in a run's worktree reaches it (`orchester_ipc::Command::RunGit`): every
//! request and answer goes through JSON, as over the runner's connection, and is carried out as the runner does
//! (`orchester-runner`'s `execution::executor::run_git`, tested in `crates/runner/tests/run_git.rs`): the run's own
//! worktree, only reads before the run ended, git run by the runner's programs. The desktop's programs note every
//! folder they ran git in, so a test sees the desktop never ran git in a run's worktree.

use std::{
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  integration::WORKTREES_DIR,
  orchestrator::run_git::{RunGitAnswer, RunGitRequest},
  store::{RunStore, WorktreeStore},
};
use orchester_exec::{programs::Programs, run_git::serve};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use orchester_workspace::{orchestrator::git::RunGit, runner::RunRequestError};
use serde::{Serialize, de::DeserializeOwned};

/// `runner`, noting the folder of every command it runs.
pub struct Noted {
  runner: Arc<dyn CommandRunner + Send + Sync>,
  folders: Mutex<Vec<PathBuf>>,
}

impl Noted {
  pub fn new(runner: Arc<dyn CommandRunner + Send + Sync>) -> Self {
    Self {
      runner,
      folders: Mutex::default(),
    }
  }

  /// Whether a command ran in `dir` or a folder in it.
  pub fn ran_in(&self, dir: &Path) -> bool {
    self
      .folders
      .lock()
      .expect("the folders")
      .iter()
      .any(|folder| folder.starts_with(dir))
  }
}

impl CommandRunner for Noted {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    if let Some(folder) = command.current_dir() {
      self.folders.lock().expect("the folders").push(folder.to_path_buf());
    }
    self.runner.run(command)
  }
}

/// The most of each of git's outputs the runner answers with, as its `MOST_OUTPUT`.
const MOST_OUTPUT: usize = orchester_ipc::frame::MAX_FRAME_LEN as usize / 16;

/// The runner of the project at `root`, running git with `programs`; it notes every request it was asked.
pub struct Runner {
  pub root: PathBuf,
  pub programs: Programs,
  pub asked: Mutex<Vec<RunGitRequest>>,
}

impl RunGit for Runner {
  fn call(&self, request: RunGitRequest) -> Result<RunGitAnswer, RunRequestError> {
    let request: RunGitRequest = over_the_wire(&request);
    self.asked.lock().expect("the requests").push(request.clone());
    let answered = self.answer(&request);
    over_the_wire(&answered).map_err(RunRequestError::Rejected)
  }
}

impl Runner {
  fn answer(&self, request: &RunGitRequest) -> Result<RunGitAnswer, Rejection> {
    let failed = |error: &dyn std::error::Error| Rejection::Failed {
      detail: error.to_string(),
    };
    let db = ProjectDb::open(&self.root).map_err(|error| failed(&error))?;
    let run = db
      .run(request.run)
      .map_err(|error| failed(&error))?
      .ok_or(Rejection::RunNotFound)?;
    if !run.status().is_terminal() && !request.allowed_before_end() {
      return Err(Rejection::RunNotEnded);
    }
    let dir = self.root.join(WORKTREES_DIR).join(run.id().to_string());
    if db.worktree_removal(run.id()).map_err(|error| failed(&error))?.is_some() || !dir.is_dir() {
      return Err(Rejection::WorktreeRemoved);
    }
    Ok(serve(&self.programs, &dir, &request.call, MOST_OUTPUT))
  }
}

/// `value` as the other side reads it from its JSON.
fn over_the_wire<T: Serialize + DeserializeOwned>(value: &T) -> T {
  let text = serde_json::to_string(value).expect("it serializes");
  serde_json::from_str(&text).expect("it reads back")
}
