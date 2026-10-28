//! The workspace a run holds while its steps follow one another, and the disk guard (ORC-114). A borrowing run takes
//! a workspace before its first step starts — waiting, like for an agent slot, while all are in use — and gives it
//! back, with a checkpoint commit of what is left, once its task no longer executes it: it waits for a person, a
//! limit or a review, paused, or ended. Its steps that worked beside others give theirs back then too.
//!
//! Below the free space the project keeps, no step starts, and agents at work are stopped with their work committed:
//! the run pauses for low disk space and goes on by itself once there is room ([`super::Scheduler`]).

use std::{path::Path, time::Duration};

use orchester_core::{
  id::StepExecutionId,
  integration::workspace::SwitchTo,
  store::WorkspaceStore,
  workspaces::{DiskVerdict, Holder},
};
use orchester_ipc::Event;
use tokio::time::Instant;

use super::{
  pool::{self, Borrowed},
  prepare::RunFolder,
  run::{ExecutionError, RunTask},
  step::worker::{Stop, StopCause},
};

/// How often the disk guard looks at the free space while agents work.
const DISK_CHECK: Duration = Duration::from_secs(30);

impl RunTask {
  /// The folder the run works in, on its branch.
  pub(super) fn worktree(&self) -> &Path {
    self
      .worktree
      .as_deref()
      .expect("a run's steps start only once it holds its folder")
  }

  /// What holds the run's next steps back, as [`RunTask::start_ready`] answers: `Some(false)` when the disk is low —
  /// nothing new starts, and the run pauses until there is room — and `Some(true)` while it waits for a workspace.
  pub(super) fn held_back(&mut self) -> Result<Option<bool>, ExecutionError> {
    if self.disk_is_low()? {
      tracing::warn!(run = %self.run.id(), "the disk is low; the run's next step waits for room");
      self.pending.low_disk = true;
      return Ok(Some(false));
    }
    Ok((!self.hold_workspace()?).then_some(true))
  }
  /// Takes the run's workspace, unless it holds one or works in its own worktree; `false` while every workspace is
  /// in use, when the run waits for one as it waits for an agent slot.
  pub(super) fn hold_workspace(&mut self) -> Result<bool, ExecutionError> {
    if self.worktree.is_some() || matches!(self.folder, RunFolder::Own(_)) {
      return Ok(true);
    }
    let holder = Holder::Run { run: self.run.id() };
    let branch = self.run.branch().clone();
    let base = self.run.base().as_str().to_owned();
    match pool::borrow(&mut self.db, &self.root, holder, (SwitchTo::Branch(&branch), &base))? {
      Borrowed::Folder(path) => {
        tracing::info!(run = %self.run.id(), folder = %path.display(), "the run took a workspace");
        self.worktree = Some(path);
        if std::mem::take(&mut self.waits_for_workspace) {
          self.announce_workspace();
        }
        Ok(true)
      }
      Borrowed::Wait(reason) => {
        if !std::mem::replace(&mut self.waits_for_workspace, true) {
          tracing::info!(run = %self.run.id(), in_use = reason.in_use, of = reason.of, "the run waits for a workspace");
          self.announce_workspace();
        }
        Ok(false)
      }
    }
  }

  /// Gives back the workspaces the run and its steps hold, each after a checkpoint commit of what is left in it: the
  /// run no longer works. A run waiting for one waits no more. What cannot be given back is logged.
  pub(super) fn give_back_workspaces(&mut self) {
    if matches!(self.folder, RunFolder::Own(_)) {
      return;
    }
    let run = self.run.id();
    if let Err(error) = self.db.end_workspace_wait(run) {
      tracing::warn!(%run, %error, "the run's wait for a workspace is still recorded");
    }
    let holders = match self.db.workspaces() {
      Ok(records) => records
        .into_iter()
        .filter_map(|record| record.holder)
        .filter(|holder| matches!(holder, Holder::Run { .. } | Holder::Step { .. }) && holder.run() == Some(run))
        .collect::<Vec<_>>(),
      Err(error) => {
        tracing::warn!(%run, %error, "the run's workspaces cannot be read to give them back");
        return;
      }
    };
    let message = format!(
      "{}: saved when its workspace was given back (run {})",
      self.task.key(),
      self.run.number()
    );
    for holder in holders {
      if let Err(error) = pool::give_back(&mut self.db, &self.root, holder, &message) {
        tracing::warn!(%run, %error, "a workspace of the run was not given back");
      }
    }
    self.worktree = None;
    self.announce_workspace();
  }

  /// Whether the disk the workspaces are on has less free space than the project keeps. The guard watches only a
  /// project that turned warm workspaces on; a disk that cannot be read holds nothing back: the guard is for space,
  /// not for errors of its own, which are logged.
  pub(super) fn disk_is_low(&self) -> Result<bool, ExecutionError> {
    let settings = self.db.workspace_settings()?;
    if !settings.enabled {
      return Ok(false);
    }
    let keep = settings.keep_free;
    match self.disks.usage(&self.root) {
      Ok(usage) => Ok(matches!(keep.verdict(usage), DiskVerdict::Low { .. })),
      Err(error) => {
        tracing::warn!(run = %self.run.id(), %error, "the disk guard cannot read the free space");
        Ok(false)
      }
    }
  }

  /// While agents work, at most every [`DISK_CHECK`]: a disk that ran low stops them with their work committed, and
  /// the run pauses for low disk space once they have ended.
  pub(super) fn check_disk_while_working(&mut self) -> Result<(), ExecutionError> {
    let now = Instant::now();
    if now < self.next_disk_check {
      return Ok(());
    }
    self.next_disk_check = now + DISK_CHECK;
    if !self.pending.low_disk && self.disk_is_low()? {
      tracing::warn!(run = %self.run.id(), "the disk ran low; the run's agents stop and it pauses");
      self.pending.low_disk = true;
      self.stop_agents(Stop::Now(StopCause::LowDisk));
    }
    Ok(())
  }

  /// Gives back the workspace of `owner`, a step that worked beside others, once its branch `branch` was merged into
  /// the run's, and deletes that branch: its commits are on the run's branch now.
  pub(super) fn give_back_merged_step(&mut self, owner: StepExecutionId, branch: &str) {
    let holder = Holder::Step {
      run: self.run.id(),
      execution: owner,
    };
    if let Err(error) = pool::give_back(&mut self.db, &self.root, holder, "merged into the run's branch") {
      tracing::warn!(run = %self.run.id(), %owner, %error, "the merged step's workspace was not given back");
    }
    // `-d` would compare with the person's checked-out branch, not the run's it was merged into.
    if let Err(error) = crate::git::Git::new(&self.root).stdout(&["branch", "--quiet", "-D", branch]) {
      tracing::warn!(run = %self.run.id(), %owner, %error, "the merged step's branch was not deleted");
    }
  }

  fn announce_workspace(&self) {
    self.notifier.send(Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
  }
}
