//! A person's review questions ([`Command::AnswerReviewQuestion`], [`Command::StopReviewAnswer`]), answered once the
//! answer's turn is under way or asked to stop.

use orchester_ipc::Command;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Answers review `command`, one of the two above.
  pub(super) fn review(&self, scheduler: Scheduler, command: &Command) -> Answer {
    match *command {
      Command::AnswerReviewQuestion { run, ref step, message } => {
        let step = step.clone();
        self.answer(run, async move {
          scheduler.answer_review_question(run, step, message).await
        })
      }
      Command::StopReviewAnswer { run, thread } => {
        self.answer(run, async move { scheduler.stop_review_answer(run, thread) })
      }
      _ => unreachable!("the server routes only the two review commands here"),
    }
  }
}
