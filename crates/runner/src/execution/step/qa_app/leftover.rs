//! What a QA app left running when its runner died: the slot file names the QA folder of its latest holder and when
//! that holder's run was created (`Lease::claim`), and whoever takes the slot next stops the programs still running
//! from that folder's `bin/`, and from its own QA folder's, before it starts its app. Its holder released the slot, so
//! nothing that runs there is in use.
//!
//! The slot file is untrusted input (any program of the user can write it; on macOS its lock is advisory), and a QA
//! folder lies in a worktree an agent changes. So a folder is looked at only when it is the runner's QA folder of an
//! Orchester worktree, `<repo>/.orchester/worktrees/<id>/.tmp/qa/run-<number>-<step>`, reached without any link: its
//! path with links resolved is the path as written, and so is its `bin/`'s. Only programs whose executables lie in
//! that `bin/` and that started since its run was created are stopped, with `stop_run_process`'s rules. The scope that
//! stops them must resolve to the `bin/` checked, so a link swapped in after the check refuses it, and once built its
//! root is not resolved again.
//!
//! Known gap: on macOS a runner killed while its script still builds leaves the script running (no kill-on-close Job
//! Object there); an app it starts after the next holder looked is not stopped by this.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{orchester_tools::ProcessCall, time::Timestamp};
use orchester_exec::run_processes::RunProcesses;

use super::slots::{Holder, Lease};

/// Claims the slot for `own`, this start's QA folder (link-free, [`link_free`]), whose run was created at `since`, and
/// stops what runs from the previous holder's QA folder and from `own`.
pub(super) fn stop(lease: &mut Lease, own: &Path, since: Timestamp) {
  let previous = lease
    .claim(&Holder {
      folder: own.to_path_buf(),
      since,
    })
    .unwrap_or_else(|error| {
      tracing::warn!(%error, "the QA slot's previous holder cannot be read; only this step's QA folder is looked at");
      None
    });
  if let Some(previous) = previous {
    match qa_folder(&previous.folder) {
      Some(folder) => stop_in(&folder, previous.since),
      None => tracing::warn!(
        folder = %previous.folder.display(),
        "the QA slot names no QA folder of an Orchester worktree reached without links; nothing is stopped there"
      ),
    }
  }
  stop_in(own, since);
}

/// `folder` with links resolved when it is the path as written: no link lies on the way to it.
pub(super) fn link_free(folder: &Path) -> Option<PathBuf> {
  let resolved = fs::canonicalize(folder).ok()?;
  (resolved == folder).then_some(resolved)
}

/// `folder` when it is a QA folder of an Orchester worktree reached without links.
fn qa_folder(folder: &Path) -> Option<PathBuf> {
  let folder = link_free(folder)?;
  let names: Vec<String> = folder
    .ancestors()
    .take(6)
    .map(|ancestor| ancestor.file_name().map(|name| name.to_string_lossy().into_owned()))
    .collect::<Option<_>>()?;
  match names.as_slice() {
    [qa, kind, tmp, _worktree, worktrees, orchester]
      if qa.starts_with("run-")
        && kind == "qa"
        && tmp == ".tmp"
        && worktrees == "worktrees"
        && orchester == ".orchester" =>
    {
      Some(folder)
    }
    _ => None,
  }
}

/// Stops every program running from `folder`'s `bin/` that started since `since`, when that `bin/` is no link.
fn stop_in(folder: &Path, since: Timestamp) {
  let bin = folder.join("bin");
  if !bin.exists() {
    return;
  }
  let Some(bin) = link_free(&bin) else {
    tracing::warn!(bin = %bin.display(), "a QA folder's bin is a link; nothing is stopped there");
    return;
  };
  // `bin` resolved to itself just now; a link swapped in since refuses the scope rather than move it elsewhere.
  let processes = match RunProcesses::checked(&bin, &bin, since) {
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
