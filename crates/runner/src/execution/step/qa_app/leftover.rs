//! What a QA app left running when its runner died: the slot file names the QA folder of its latest holder
//! (`Lease::claim`), and whoever takes the slot next stops the programs still running from that folder's `bin/`, and
//! from its own QA folder's, before it starts its app. Its holder released the slot, so nothing that runs there is in
//! use. Only programs whose executables lie in that `bin/` are stopped, with `stop_run_process`'s rules.
//!
//! Known gap: on macOS a runner killed while its script still builds leaves the script running (no kill-on-close Job
//! Object there); an app it starts after the next holder looked is not stopped by this.

use std::path::Path;

use orchester_core::{orchester_tools::ProcessCall, time::Timestamp};
use orchester_exec::run_processes::RunProcesses;

use super::slots::Lease;

/// Claims the slot for `folder` and stops what runs from the previous holder's QA folder and from `folder`.
pub(super) fn stop(lease: &mut Lease, folder: &Path) {
  let previous = lease.claim(folder).unwrap_or_else(|error| {
    tracing::warn!(%error, "the QA slot's previous holder cannot be read; only this step's QA folder is looked at");
    None
  });
  for qa in previous.iter().filter(|previous| is_qa_folder(previous)) {
    stop_in(&qa.join("bin"));
  }
  stop_in(&folder.join("bin"));
}

/// Whether `path` is a QA folder the runner names, `<worktree>/.tmp/qa/run-<number>-<step>`.
fn is_qa_folder(path: &Path) -> bool {
  let name = |path: Option<&Path>| {
    path
      .and_then(Path::file_name)
      .and_then(|name| name.to_str())
      .map(str::to_owned)
  };
  name(Some(path)).is_some_and(|name| name.starts_with("run-"))
    && name(path.parent()).as_deref() == Some("qa")
    && name(path.parent().and_then(Path::parent)).as_deref() == Some(".tmp")
}

/// Stops every program running from `bin`, whenever it started.
fn stop_in(bin: &Path) {
  if !bin.is_dir() {
    return;
  }
  let processes = match RunProcesses::new(bin, Timestamp::from_unix_millis(0)) {
    Ok(processes) => processes,
    Err(error) => {
      tracing::warn!(%error, bin = %bin.display(), "a QA folder's programs cannot be looked at");
      return;
    }
  };
  let pids = processes.pids().unwrap_or_else(|error| {
    tracing::warn!(%error, "the running programs cannot be listed");
    Vec::new()
  });
  for pid in pids {
    let stopped = processes.run(ProcessCall::Stop { pid });
    if stopped.is_error() {
      tracing::warn!(pid, answer = ?stopped, "a QA app left running was not stopped");
    } else {
      tracing::info!(pid, bin = %bin.display(), "stopped a QA app left running");
    }
  }
}
