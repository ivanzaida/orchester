//! A run's branch rebased onto a newer commit of the branch it merges into after the run started,
//! as ORC-39's was. The run keeps the base it started from, so the commits between that base and
//! the new one, which the target has, would read as the run's own work already integrated. A
//! rebase recorded here moves the base external evidence reads the run's commits from, only when
//! git proves it ([`RebaseEvidence`]). Of the run's own commits below the new base, only the
//! checkpointed ones the target has by ancestry keep the base where it was; one the target has as a
//! pick, or one never checkpointed, is not told apart from the target's own.

use crate::{id::RunId, revision::Revision, time::Timestamp};

/// A last checkpoint the rebase rewrote, and the branch tip whose own commits are exactly its
/// rebased form: the tip stands for the checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebasedCheckpoint {
  pub replaces: Revision,
  pub tip: Revision,
}

/// What git says of a run's branch, read for [`RunRebase::proven`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebaseEvidence {
  /// The commit the branch's newest rebase put it onto, as the branch's reflog names it.
  pub onto: Revision,
  /// The run's recorded base is an ancestor of `onto`, and not `onto` itself.
  pub newer_than_base: bool,
  /// `onto` is an ancestor of the branch tip and of the branch the run merges into.
  pub under_tip_and_target: bool,
  /// A checkpoint of the run lies between its recorded base and `onto`: then commits of the run's
  /// own are below `onto`, and moving the base would hide that the target has them.
  pub own_work_below: bool,
  /// The last checkpoint, when the rebase rewrote it into the branch tip.
  pub checkpoint: Option<RebasedCheckpoint>,
}

/// A rebase of a run's branch that git proved: the base its own commits sit on now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRebase {
  run: RunId,
  base: Revision,
  checkpoint: Option<RebasedCheckpoint>,
  recorded_at: Timestamp,
}

impl RunRebase {
  /// The rebase of `run` that `evidence` proves, recorded at `now`; `None` when it proves none.
  pub fn proven(run: RunId, evidence: RebaseEvidence, now: Timestamp) -> Option<Self> {
    let proven = evidence.newer_than_base && evidence.under_tip_and_target && !evidence.own_work_below;
    proven.then_some(Self {
      run,
      base: evidence.onto,
      checkpoint: evidence.checkpoint,
      recorded_at: now,
    })
  }

  /// A rebase as it was recorded.
  pub const fn restore(
    run: RunId,
    base: Revision,
    checkpoint: Option<RebasedCheckpoint>,
    recorded_at: Timestamp,
  ) -> Self {
    Self {
      run,
      base,
      checkpoint,
      recorded_at,
    }
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  pub const fn base(&self) -> &Revision {
    &self.base
  }

  pub const fn checkpoint(&self) -> Option<&RebasedCheckpoint> {
    self.checkpoint.as_ref()
  }

  pub const fn recorded_at(&self) -> Timestamp {
    self.recorded_at
  }

  /// The run's last checkpoint as its branch holds it now: the tip, where the rebase rewrote
  /// `last`; else `last`.
  pub fn checkpoint_for<'a>(&'a self, last: Option<&'a Revision>) -> Option<&'a Revision> {
    match &self.checkpoint {
      Some(rebased) if last == Some(&rebased.replaces) => Some(&rebased.tip),
      _ => last,
    }
  }

  /// Whether this records the same rebase as `other`, whenever each was recorded.
  pub fn same_as(&self, other: &Self) -> bool {
    self.run == other.run && self.base == other.base && self.checkpoint == other.checkpoint
  }
}

/// Where proven rebases of runs' branches are kept, one per run: a newer one replaces it.
pub trait RebaseStore {
  type Error: std::error::Error + Send + Sync + 'static;

  fn save_run_rebase(&mut self, rebase: &RunRebase) -> Result<(), Self::Error>;

  fn run_rebase(&self, run: RunId) -> Result<Option<RunRebase>, Self::Error>;
}
