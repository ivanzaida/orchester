//! Each project's answers sent from this window that its tasks were not read back with yet (`tasks::answering`):
//! marked when an answer is sent, settled by the runner's reply, and let go once a reading of the tasks tells what
//! became of them. One set per project, kept with its run counters, so every window surface and automation share it.

use std::path::Path;

use tokio::sync::watch;

use super::Runners;
use crate::tasks::answering::{Answered, PendingAnswers};

impl Runners {
  /// Project `root`'s pending answers, whenever they change.
  pub fn answers_of(&self, root: &Path) -> watch::Receiver<PendingAnswers> {
    self.inner.shared.counters(root).answers.subscribe()
  }

  /// Project `root`'s pending answers now.
  pub fn answers_now(&self, root: &Path) -> PendingAnswers {
    self.inner.shared.counters(root).answers.borrow().clone()
  }

  /// A reading of project `root`'s tasks begun once `settled` of its answers had settled ended: the marks it tells
  /// the end of go.
  pub fn answers_read(&self, root: &Path, settled: u64) {
    self
      .inner
      .shared
      .counters(root)
      .answers
      .send_if_modified(|answers| answers.read_at(settled));
  }

  /// Marks `answered` as answered in project `root` now, until the returned mark settles.
  pub fn answering(&self, root: &Path, answered: Answered) -> AnswerMark {
    let answers = self.inner.shared.counters(root).answers;
    let mut id = 0;
    answers.send_modify(|pending| id = pending.mark(answered));
    AnswerMark {
      answers,
      id,
      settled: false,
    }
  }
}

/// An answer marked as answered ([`Runners::answering`]). One dropped without being settled settles as taken: what
/// became of it is not known, so the next reading of the tasks, which its settling starts, tells.
pub struct AnswerMark {
  answers: watch::Sender<PendingAnswers>,
  id: u64,
  settled: bool,
}

impl AnswerMark {
  /// The runner took the answer (`taken`), and the next reading drops it for good; or it did not, and the item waits
  /// again now.
  pub fn settle(mut self, taken: bool) {
    self.settle_as(taken);
  }

  fn settle_as(&mut self, taken: bool) {
    self.settled = true;
    let id = self.id;
    self.answers.send_modify(|pending| pending.settle(id, taken));
  }
}

impl Drop for AnswerMark {
  fn drop(&mut self) {
    if !self.settled {
      self.settle_as(true);
    }
  }
}
