//! How a recorded run stops, as the runner records it: paused, at a limit, interrupted, cancelled or stopped by the
//! person, and how it ends once merged, its worktree removed.

use orchester_core::{
  connection::stream::InvocationOutcome,
  run::{InterruptCause, RunLimit, StuckDecision},
  store::{MergeStore, WorktreeStore},
  tokens::TokensUsed,
};

use super::{
  super::{SeedError, git},
  Drive,
};

impl Drive<'_> {
  /// Ends the invocations at work, which the runner stops when a run pauses, stops or is cancelled.
  pub(super) fn stop_open(&mut self) -> Result<(), SeedError> {
    let steps: Vec<String> = self.open.keys().map(|key| key.as_str().to_owned()).collect();
    for step in steps {
      self.end(&step, Vec::new(), InvocationOutcome::Terminated)?;
    }
    Ok(())
  }

  pub fn pause(&mut self) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    let events = self.run.pause(at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Stops at `limit`; a run's own token limit is used up first, as the runner stops it when it is.
  pub fn stop_at_limit(&mut self, limit: RunLimit) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    if limit == RunLimit::Tokens {
      let budget = self.run.budget().ok_or(SeedError::NoBudget)?;
      let left = budget.get().saturating_sub(self.run.used().get());
      let events = self.run.record_usage(TokensUsed::new(left), at)?;
      self.record(&events)?;
    }
    let events = self.run.stop_at_limit(limit, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// The app ended while the run worked: no end of its invocations was recorded.
  pub fn interrupt(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let events = self.run.interrupt(InterruptCause::Crashed, at)?;
    self.record(&events)?;
    Ok(self)
  }

  pub fn cancel(&mut self) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    let events = self.run.cancel(at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// The person stops a run that waits for them: it fails.
  pub fn stop(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let events = self.run.decide_stuck(&StuckDecision::Stop, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Merge into main, then Remove worktree.
  pub fn merge(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let merge = git::merge(&self.kontur.repo, (&self.run, self.task.key()), at)?;
    self.kontur.db.save_run_merge(&merge)?;
    self.remove_worktree()
  }

  /// Removes the run's worktree; its branch and commits stay.
  pub fn remove_worktree(&mut self) -> Result<&mut Self, SeedError> {
    let worktree = self.worktree.take().ok_or(SeedError::WorktreeRemoved)?;
    worktree.remove()?;
    let at = self.tick();
    self.kontur.db.save_worktree_removal(self.run.id(), at)?;
    Ok(self)
  }
}
