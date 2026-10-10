//! Blocking process tools for one invocation. Core resolves the executable boundary; platform modules inspect and
//! signal individual processes. Every child passes the same rule, so a tree is never a grant to kill outside it.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod windows;

use std::{
  collections::BTreeSet,
  io,
  num::NonZeroU32,
  path::{Path, PathBuf},
  thread,
  time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use macos as platform;
use orchester_core::orchester_tools::{ProcessCall, ProcessScopeError, RunProcessScope, ToolResult};
use parking_lot::Mutex;
use serde_json::json;
use thiserror::Error;
#[cfg(not(any(windows, target_os = "macos")))]
use unsupported as platform;
#[cfg(windows)]
use windows as platform;

const GRACE: Duration = Duration::from_secs(2);
const END_WAIT: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);
const MAX_TREE: usize = 1024;

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

#[derive(Debug, Error)]
enum ProcessError {
  #[error(transparent)]
  Scope(#[from] ProcessScopeError),
  #[error("process inspection or stop failed: {0}")]
  Os(#[from] io::Error),
  #[error("refused pid {pid}: {reason}; no process outside this run's worktree can be stopped")]
  Refused { pid: u32, reason: String },
  #[error("pid {0} did not end within the bounded wait")]
  StillRunning(u32),
}

/// The process executor of a relay, with the worktree and protected programs pinned at invocation start.
pub struct RunProcesses {
  scope: RunProcessScope,
  protected: Mutex<Vec<PathBuf>>,
}

impl RunProcesses {
  pub fn new(worktree: &Path) -> Result<Self, ProcessScopeError> {
    Ok(Self {
      scope: RunProcessScope::new(worktree)?,
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

  fn list(&self) -> Result<String, ProcessError> {
    let mut listed = Vec::new();
    for entry in platform::snapshot()? {
      let process = match platform::Process::open(entry, false) {
        Ok(Some(process)) => process,
        Ok(None) => continue,
        // The OS hides other users' processes. They cannot be offered or stopped without a verified path.
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => continue,
        Err(error) => return Err(error.into()),
      };
      match self.scope.relative_executable(&process.info.path) {
        Ok(Some(relative)) => listed.push(json!({
          "pid":process.info.pid,
          "name":process.info.path.file_name().map(|name| name.to_string_lossy()),
          "path":relative.to_string_lossy().replace('\\', "/"),
          "start_time_unix_micros":process.info.started,
        })),
        Ok(None) => {}
        // An executable may disappear while listing; it cannot pass a resolved-path rule.
        Err(_) => {}
      }
    }
    listed.sort_by_key(|process| process["pid"].as_u64());
    Ok(json!(listed).to_string())
  }

  fn validate(&self, process: &platform::Process, protected_pids: &BTreeSet<u32>) -> Result<(), ProcessError> {
    let pid = process.info.pid;
    let refused = |reason: String| ProcessError::Refused { pid, reason };
    let canonical = std::fs::canonicalize(&process.info.path)
      .map_err(|error| refused(format!("cannot resolve executable: {error}")))?;
    if protected_pids.contains(&pid) || self.protected.lock().contains(&canonical) {
      return Err(refused(
        "the runner, its ancestors and the agent CLI are protected".to_owned(),
      ));
    }
    if self.scope.relative_executable(&process.info.path)?.is_none() {
      return Err(refused(format!(
        "executable {} is outside the worktree",
        process.info.path.display()
      )));
    }
    Ok(())
  }

  fn stop(&self, pid: NonZeroU32) -> Result<String, ProcessError> {
    let pid = pid.get();
    let Some(root) = platform::Process::open(Entry::pid(pid), true).map_err(|error| ProcessError::Refused {
      pid,
      reason: format!("cannot verify executable: {error}"),
    })?
    else {
      return Ok(format!("pid {pid}: already gone"));
    };
    let entries = platform::snapshot()?;
    let protected = ancestors(&entries);
    self.validate(&root, &protected)?;
    let mut tree = vec![root];
    self.discover(&mut tree, &entries, &protected)?;
    let mut close_errors = Vec::new();
    for process in tree.iter().rev() {
      self.request_close(process, &protected, &mut close_errors)?;
    }
    let deadline = Instant::now() + GRACE;
    while Instant::now() < deadline {
      let old_len = tree.len();
      self.discover(&mut tree, &platform::snapshot()?, &protected)?;
      for process in &tree[old_len..] {
        self.request_close(process, &protected, &mut close_errors)?;
      }
      if all_gone(&tree)? {
        break;
      }
      thread::sleep(POLL);
    }
    let mut report = self.finish_tree(pid, &mut tree, &protected, &mut close_errors)?;
    if !close_errors.is_empty() {
      report.push_str(&format!("\n{}", close_errors.join("\n")));
    }
    Ok(report)
  }

  fn request_close(
    &self,
    process: &platform::Process,
    protected: &BTreeSet<u32>,
    errors: &mut Vec<String>,
  ) -> Result<(), ProcessError> {
    if !process.alive()? {
      return Ok(());
    }
    self.validate(process, protected)?;
    if let Err(error) = process.close() {
      errors.push(format!(
        "pid {}: close request failed ({error}); attempted to end after waiting",
        process.info.pid
      ));
    }
    Ok(())
  }

  fn discover(
    &self,
    tree: &mut Vec<platform::Process>,
    entries: &[Entry],
    protected: &BTreeSet<u32>,
  ) -> Result<(), ProcessError> {
    let mut index = 0;
    while index < tree.len() {
      let parent = &tree[index];
      let (parent_pid, parent_started) = (parent.info.pid, parent.info.started);
      // Retain exited parents: a close handler can spawn a child and exit between snapshots.
      for entry in entries {
        if !tree[index].is_parent_of(*entry) {
          continue;
        }
        if tree.iter().any(|process| process.is_process(*entry)) {
          continue;
        }
        let Some(child) = platform::Process::open(*entry, true)? else {
          continue;
        };
        // A stale parent PID in a snapshot does not establish descent from a newer process.
        if child.info.started < parent_started {
          continue;
        }
        self.validate(&child, protected)?;
        if tree.len() >= MAX_TREE {
          return Err(ProcessError::Refused {
            pid: parent_pid,
            reason: "process tree exceeded its bound".to_owned(),
          });
        }
        tree.push(child);
      }
      index += 1;
    }
    Ok(())
  }

  fn finish_tree(
    &self,
    pid: u32,
    tree: &mut Vec<platform::Process>,
    protected: &BTreeSet<u32>,
    close_errors: &mut Vec<String>,
  ) -> Result<String, ProcessError> {
    let mut ended = vec![false; tree.len()];
    let deadline = Instant::now() + END_WAIT;
    loop {
      for (process, ended) in tree.iter().zip(&mut ended).rev() {
        if !*ended && process.alive()? {
          self.validate(process, protected)?;
          process.end()?;
          *ended = true;
        }
      }
      let gone = all_gone(tree)?;
      let old_len = tree.len();
      // Scan after checking exits, so the final snapshot includes children born before those exits.
      self.discover(tree, &platform::snapshot()?, protected)?;
      if gone && old_len == tree.len() {
        return Ok(
          tree
            .iter()
            .zip(ended)
            .map(|(process, ended)| format!("pid {}: {}", process.info.pid, if ended { "ended" } else { "closed" }))
            .collect::<Vec<_>>()
            .join("\n"),
        );
      }
      for process in &tree[old_len..] {
        self.request_close(process, protected, close_errors)?;
      }
      ended.resize(tree.len(), false);
      if Instant::now() >= deadline {
        return Err(ProcessError::StillRunning(pid));
      }
      // Even a child first seen during escalation gets a close request and a bounded wait.
      thread::sleep(POLL);
    }
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

fn all_gone(tree: &[platform::Process]) -> io::Result<bool> {
  for process in tree {
    if process.alive()? {
      return Ok(false);
    }
  }
  Ok(true)
}
