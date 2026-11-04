//! Routes answers to agents' questions with suggested answers (`orchester_core::question`) inside an executing
//! run; the scheduler's side, which checks an answer against the questions first, is
//! `crate::execution::coordinator::question`'s. A question an agent waits on in its turn goes to the live worker
//! holding it; one no live agent holds can never be answered and ends as the answer is refused. A native agent's
//! question ended its step: the answer is recorded with the person's message that carries it to the agent, and the
//! step resumes as it does for any message that answers a question. While such a question waits, other messages do
//! not resume its step ([`held_by_question`]): they reach the agent with the answer.

use orchester_core::{
  id::{RunId, StepExecutionId},
  question::{AskedQuestions, GivenAnswer, QuestionState, QuestionTurn},
  run::Answerer,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, QuestionRecord, StoreError};
use tokio::sync::mpsc::error::SendError;

use super::{
  control::{Ack, answer},
  run::{ExecutionError, RunTask},
  sink::Sink,
  step::question::QuestionAnswer,
};

/// Whether a question with suggested answers that ended `execution`'s step still waits: messages written
/// meanwhile do not resume the step, they reach the agent with the answer.
pub(in crate::execution) fn held_by_question(
  db: &ProjectDb,
  run: RunId,
  execution: StepExecutionId,
) -> Result<bool, StoreError> {
  Ok(db.questions(run)?.iter().any(|asked| {
    asked.waits() && asked.request.turn() == QuestionTurn::StepEnd && asked.request.execution() == execution
  }))
}

/// Ends request `id` of `run`, which no live agent holds, through `sink` if it still waits: whether it did, and why
/// its answer is refused, read in `db`.
pub(in crate::execution) fn end_unheld(
  (db, sink): (&ProjectDb, &mut dyn Sink),
  run: RunId,
  id: &str,
) -> Result<(bool, Rejection), StoreError> {
  if sink.question(QuestionRecord::ended(run, id))? {
    return Ok((true, Rejection::QuestionNotAwaited));
  }
  Ok((false, refusal(db, run, id)?))
}

/// Why an answer to request `id` of `run`, which no longer waits for this answer, is refused.
pub(in crate::execution) fn refusal(db: &ProjectDb, run: RunId, id: &str) -> Result<Rejection, StoreError> {
  Ok(match db.question_request(run, id)?.map(|asked| asked.state) {
    Some(QuestionState::Answered { .. }) => Rejection::QuestionAnswered,
    _ => Rejection::QuestionNotAwaited,
  })
}

impl RunTask {
  pub(super) fn answer_question(
    &mut self,
    request: String,
    (answers, by): (Vec<GivenAnswer>, Answerer),
    ack: Ack,
  ) -> Result<(), ExecutionError> {
    let asked = self.db.question_request(self.run.id(), &request)?;
    let Some(asked) = asked.filter(AskedQuestions::waits) else {
      answer(ack, Err(refusal(&self.db, self.run.id(), &request)?));
      return Ok(());
    };
    let worker = self
      .agents
      .get(&asked.request.execution())
      .and_then(|active| active.questions.as_ref());
    let Some(worker) = worker else {
      return self.end_unheld_question(&request, ack);
    };
    if let Err(SendError(given)) = worker.send(QuestionAnswer {
      request,
      answers,
      by,
      ack,
    }) {
      tracing::debug!(run = %self.run.id(), "an answer arrived after the agent's worker ended");
      return self.end_unheld_question(&given.request, given.ack);
    }
    Ok(())
  }

  fn end_unheld_question(&mut self, id: &str, ack: Ack) -> Result<(), ExecutionError> {
    let (ended, refusal) = end_unheld((&self.db, self.sink.as_mut()), self.run.id(), id)?;
    if ended {
      self.notifier.send(Event::QuestionChanged {
        run: self.run.id(),
        task: self.task.id(),
      });
    }
    answer(ack, Err(refusal));
    Ok(())
  }

  /// The run's task no longer executes it: the questions still waiting in its agents' turns can never be
  /// answered, and end.
  pub(super) fn end_waiting_questions(&mut self) {
    let run = self.run.id();
    let ended = self.db.questions(run).and_then(|asked| {
      asked
        .iter()
        .filter(|asked| asked.waits() && asked.request.turn() == QuestionTurn::InTurn)
        .try_fold(false, |ended, asked| {
          Ok(self.sink.question(QuestionRecord::ended(run, asked.request.id()))? || ended)
        })
    });
    match ended {
      Ok(false) => {}
      Ok(true) => self.notifier.send(Event::QuestionChanged {
        run,
        task: self.task.id(),
      }),
      // The runner keeps going; the next runner to attach ends them.
      Err(error) => tracing::error!(%run, %error, "cannot end the questions left waiting"),
    }
  }
}
