//! Blocking process tools for one invocation. Core resolves the executable boundary; platform modules inspect and
//! signal individual processes. A stop reaches the root and the descendants inside the worktree; a descendant
//! outside it is never closed or ended, only named in the report.

mod app;
#[cfg(target_os = "macos")]
mod macos;
mod stop;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod windows;

use std::{
  collections::BTreeSet,
  fmt, io,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

pub use app::{RunAppProcess, RunAppProcessError};
#[cfg(target_os = "macos")]
use macos as platform;
use orchester_core::{
  orchester_tools::{ProcessCall, ProcessScopeError, RunProcessScope, ToolResult},
  time::Timestamp,
};
use parking_lot::Mutex;
use serde_json::json;
use thiserror::Error;
#[cfg(not(any(windows, target_os = "macos")))]
use unsupported as platform;
#[cfg(windows)]
use windows as platform;

#[derive(Clone, Copy)]
struct Entry {
  pid: u32,
  parent: u32,
  #[cfg(target_os = "macos")]
  unique_id: u64,
  #[cfg(target_os = "macos")]
  parent_unique_id: u64,
}

impl Entry {
  fn pid(pid: u32) -> Self {
    Self {
      pid,
      parent: 0,
      #[cfg(target_os = "macos")]
      unique_id: 0,
      #[cfg(target_os = "macos")]
      parent_unique_id: 0,
    }
  }
}

struct Info {
  pid: u32,
  path: PathBuf,
  started: u64,
}

impl Info {
  /// The executable's file name only: a report never reveals where a program outside the worktree lives.
  fn name(&self) -> String {
    self
      .path
      .file_name()
      .map_or_else(|| "unknown".to_owned(), |name| name.to_string_lossy().into_owned())
  }
}

/// Why a process is not stopped. None of them names a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
  Protected,
  StartedBeforeRun,
  Outside,
  Unreadable,
  TreeBound,
}

impl fmt::Display for Refusal {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(match self {
      Self::Protected => "the runner, its ancestors, the runner's executable and the agent CLI are protected",
      Self::StartedBeforeRun => "it started before this run",
      Self::Outside => "its executable is outside this run's worktree",
      Self::Unreadable => "its executable cannot be read",
      Self::TreeBound => "its process tree exceeds the bound of processes one stop handles",
    })
  }
}

#[derive(Debug, Error)]
enum ProcessError {
  #[error("process inspection failed: {0}")]
  Os(#[from] io::Error),
  #[error("refused pid {pid}: {refusal}; no process outside this run's worktree can be stopped")]
  Refused { pid: u32, refusal: Refusal },
  #[error("not every process of pid {pid} inside this run's worktree was stopped:\n{report}")]
  Incomplete { pid: u32, report: String },
}

/// The process executor of a relay, with the worktree and protected programs pinned at invocation start.
pub struct RunProcesses {
  scope: RunProcessScope,
  protected: Mutex<Vec<PathBuf>>,
}

impl RunProcesses {
  /// The executor of the run working in `worktree`, created at `run_created` ([`RunProcessScope::new`]).
  pub fn new(worktree: &Path, run_created: Timestamp) -> Result<Self, ProcessScopeError> {
    Ok(Self {
      scope: RunProcessScope::new(worktree, run_created)?,
      protected: Mutex::new(Vec::new()),
    })
  }

  /// The executor of programs in `folder`, which resolved to `checked` when its caller looked
  /// ([`RunProcessScope::checked`]): refused when it resolves elsewhere now.
  pub fn checked(folder: &Path, checked: &Path, run_created: Timestamp) -> Result<Self, ProcessScopeError> {
    Ok(Self {
      scope: RunProcessScope::checked(folder, checked, run_created)?,
      protected: Mutex::new(Vec::new()),
    })
  }

  /// Protects the runner or agent CLI by executable, including a copy launched from inside the worktree.
  pub fn protect_executable(&self, executable: &Path) -> io::Result<()> {
    self.protected.lock().push(std::fs::canonicalize(executable)?);
    Ok(())
  }

  pub fn run(&self, call: ProcessCall) -> ToolResult {
    let result = match call {
      ProcessCall::List => self.list(),
      ProcessCall::Stop { pid } => self.stop(pid),
    };
    match result {
      Ok(text) => ToolResult::shown(text),
      Err(error) => ToolResult::unavailable(&error.to_string()),
    }
  }

  /// The pids of the processes `list_run_processes` lists: started since the scope's start, their executables inside
  /// it.
  pub fn pids(&self) -> io::Result<Vec<NonZeroU32>> {
    let mut pids = Vec::new();
    for entry in platform::snapshot()? {
      let Ok(Some(process)) = platform::Process::open(entry, false) else {
        continue;
      };
      if self.scope.started_in_run(process.info.started)
        && let Ok(Some(_)) = self.scope.relative_executable(&process.info.path)
        && let Some(pid) = NonZeroU32::new(process.info.pid)
      {
        pids.push(pid);
      }
    }
    Ok(pids)
  }

  fn list(&self) -> Result<String, ProcessError> {
    let mut listed = Vec::new();
    for entry in platform::snapshot()? {
      // A process that exits, belongs to another user or is protected by the OS (System, Registry) cannot be
      // offered: only a process whose executable can be read and verified is listed.
      let Ok(Some(process)) = platform::Process::open(entry, false) else {
        continue;
      };
      if !self.scope.started_in_run(process.info.started) {
        continue;
      }
      // An executable may disappear while listing; it cannot pass a resolved-path rule.
      if let Ok(Some(relative)) = self.scope.relative_executable(&process.info.path) {
        listed.push(json!({
          "pid":process.info.pid,
          "name":process.info.name(),
          "path":relative.to_string_lossy().replace('\\', "/"),
          "start_time_unix_micros":process.info.started,
        }));
      }
    }
    listed.sort_by_key(|process| process["pid"].as_u64());
    Ok(json!(listed).to_string())
  }

  /// Whether this process may be stopped; the image path is pinned by the retained handle (Windows) or audit token
  /// (macOS), so one check holds for the whole stop.
  fn validate(&self, process: &platform::Process, protected_pids: &BTreeSet<u32>) -> Result<(), Refusal> {
    if protected_pids.contains(&process.info.pid) {
      return Err(Refusal::Protected);
    }
    if !self.scope.started_in_run(process.info.started) {
      return Err(Refusal::StartedBeforeRun);
    }
    match self.scope.relative_executable(&process.info.path) {
      Ok(Some(_)) => {}
      Ok(None) => return Err(Refusal::Outside),
      Err(_) => return Err(Refusal::Unreadable),
    }
    let canonical = std::fs::canonicalize(&process.info.path).map_err(|_| Refusal::Unreadable)?;
    if self.protected.lock().contains(&canonical) {
      return Err(Refusal::Protected);
    }
    Ok(())
  }

  fn stop(&self, pid: NonZeroU32) -> Result<String, ProcessError> {
    let pid = pid.get();
    let root = match platform::Process::open(Entry::pid(pid), true) {
      Ok(Some(root)) => root,
      Ok(None) => return Ok(format!("pid {pid}: already gone")),
      Err(_) => {
        return Err(ProcessError::Refused {
          pid,
          refusal: Refusal::Unreadable,
        });
      }
    };
    let entries = platform::snapshot()?;
    let protected = ancestors(&entries);
    self
      .validate(&root, &protected)
      .map_err(|refusal| ProcessError::Refused { pid, refusal })?;
    let mut stop = stop::Stop::new(self, protected, root);
    if stop.discover(&entries).is_err() {
      return Err(ProcessError::Refused {
        pid,
        refusal: Refusal::TreeBound,
      });
    }
    stop.run(pid)
  }
}

fn ancestors(entries: &[Entry]) -> BTreeSet<u32> {
  let mut result = BTreeSet::new();
  let mut current = std::process::id();
  while current != 0 && result.insert(current) {
    current = entries
      .iter()
      .find(|entry| entry.pid == current)
      .map_or(0, |entry| entry.parent);
  }
  result
}
