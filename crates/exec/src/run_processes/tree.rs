//! The processes of one program's tree that a UI driver may reach (`docs/architecture/agent-testing.md`, "UI
//! drivers"): the program and its descendants, each checked as `stop_run_process` checks a process before it stops
//! it — started since the run began, its executable inside the run's worktree, not the runner or the agent CLI. Each
//! process kept is held open while the tree lives, so its pid cannot name another process meanwhile.

use std::{io, num::NonZeroU32};

use super::{Entry, RunProcesses, ancestors, platform};

/// The most processes one tree follows.
const MAX_TREE: usize = 256;

/// The checked processes of a program's tree.
pub struct RunTree {
  /// The processes the run may reach, the program first.
  reached: Vec<platform::Process>,
  /// The others, each with why it is not reached.
  refused: Vec<String>,
}

impl RunTree {
  /// The pids of the processes the run may reach.
  pub fn pids(&self) -> Vec<u32> {
    self.reached.iter().map(|process| process.info.pid).collect()
  }

  /// Why each of the tree's other processes is not reached, one line each, naming its pid and program only.
  pub fn refused(&self) -> &[String] {
    &self.refused
  }
}

impl RunProcesses {
  /// The tree of the program `root`, its processes checked one by one; empty when it is gone.
  pub fn tree(&self, root: NonZeroU32) -> io::Result<RunTree> {
    let entries = platform::snapshot()?;
    let protected = ancestors(&entries);
    let mut found = Vec::new();
    if let Some(process) = platform::Process::open(Entry::pid(root.get()), false)? {
      found.push(process);
    }
    let mut index = 0;
    while index < found.len() && found.len() < MAX_TREE {
      let parent_started = found[index].info.started;
      for entry in &entries {
        if !found[index].is_parent_of(*entry) || found.iter().any(|known| known.is_process(*entry)) {
          continue;
        }
        // A child that exits or cannot be opened is not reached; a stale parent pid proves no descent.
        if let Ok(Some(child)) = platform::Process::open(*entry, false)
          && child.info.started >= parent_started
        {
          found.push(child);
        }
      }
      index += 1;
    }
    let mut tree = RunTree {
      reached: Vec::new(),
      refused: Vec::new(),
    };
    for process in found {
      match self.validate(&process, &protected) {
        Ok(()) => tree.reached.push(process),
        Err(refusal) => tree
          .refused
          .push(format!("pid {} ({}): {refusal}", process.info.pid, process.info.name())),
      }
    }
    Ok(tree)
  }
}
