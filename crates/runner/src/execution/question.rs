//! Routes answers to agents' questions with suggested answers (`orchester_core::question`), from the app's
//! card and from Orchester's MCP server alike. An answer is checked against the questions first. A question
//! an agent waits on in its turn goes to the live worker holding it; one no live agent holds can never be
//! answered and ends as the answer is refused. A native agent's question ended its step: the answer is
//! recorded with the person's message that carries it to the agent, and the step resumes as it does for any
//! message that answers a question. While such a question waits, other messages do not resume its step
//! ([`held_by_question`]): they reach the agent with the answer.

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId, StepExecutionId, TaskId},
  question::{Answer, AskedQuestions, GivenAnswer, QuestionRequest, QuestionState, QuestionTurn, answers_for_agent},
  run::{Answerer, StepExecutor, StepStatus, WaitReason},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};
use tokio::sync::mpsc::error::SendError;

use super::{
  Notifier, Scheduler,
  control::{Ack, Control, answer},
  failed, idle,
  run::{ExecutionError, RunTask},
  step::question::QuestionAnswer,
};

impl Scheduler {
  pub async fn answer_question(
    &self,
    run: RunId,
    request: String,
    answers: Vec<GivenAnswer>,
    by: Answerer,
  ) -> Result<(), Rejection> {
    let mut db = self.open()?;
    let asked = db
      .question_request(run, &request)
      .map_err(|error| failed("cannot read the question", &error))?
      .ok_or(Rejection::QuestionNotAwaited)?;
    match asked.state {
      QuestionState::Waiting => {}
      QuestionState::Answered { .. } => return Err(Rejection::QuestionAnswered),
      QuestionState::Interrupted { .. } => return Err(Rejection::QuestionNotAwaited),
    }
    let checked = asked
      .request
      .answer(&answers)
      .map_err(|error| Rejection::InvalidAnswer { error })?;
    if asked.request.turn() == QuestionTurn::StepEnd {
      let task = answer_step_end(&mut db, &asked.request, &checked, by)?;
      self.inner.notifier.send(Event::QuestionChanged { run, task });
      return self.deliver_messages(run).await;
    }
    let Some(reply) = self.ask(run, |ack| Control::AnswerQuestion {
      request: request.clone(),
      answers,
      by,
      ack,
    }) else {
      return Err(self.end_unheld_question(&mut db, run, &request));
    };
    // A task that drops the answer has no agent left that could hold the question.
    match reply.await {
      Ok(result) => result,
      Err(_) => Err(self.end_unheld_question(&mut db, run, &request)),
    }
  }

  /// No task executes `run`, so none of its agents waits in its turn: question request `id` ends if it
  /// still waits, and the answer is refused.
  fn end_unheld_question(&self, db: &mut ProjectDb, run: RunId, id: &str) -> Rejection {
    match end_unheld(db, run, id) {
      Ok((ended, refusal)) => {
        if ended {
          announce(db, &self.inner.notifier, run);
        }
        refusal
      }
      Err(error) => failed("cannot end a question no agent holds", &error),
    }
  }

  /// Ends the questions left waiting in the turns of agents that are gone: those of every run this runner
  /// does not execute. Called as a runner attaches.
  pub(super) fn end_unheld_questions(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let store = |error: &dyn std::error::Error| failed("cannot end the questions left waiting", error);
    let unheld: Vec<AskedQuestions> = db
      .waiting_questions()
      .map_err(|error| store(&error))?
      .into_iter()
      .filter(|asked| asked.request.turn() == QuestionTurn::InTurn && !self.is_active(asked.request.run()))
      .collect();
    for asked in unheld {
      let run = asked.request.run();
      if db
        .end_waiting_question(run, asked.request.id())
        .map_err(|error| store(&error))?
      {
        tracing::info!(%run, "a question left waiting by a gone agent ended");
        announce(db, &self.inner.notifier, run);
      }
    }
    Ok(())
  }
}

/// Records `answers` to step-end request `request`, `by` its answerer, with the person's message that
/// carries them to the asking agent; the run's task. Refused when the step no longer waits for it.
fn answer_step_end(
  db: &mut ProjectDb,
  request: &QuestionRequest,
  answers: &[Answer],
  by: Answerer,
) -> Result<TaskId, Rejection> {
  let store = |error: StoreError| failed("cannot record the answer", &error);
  let run = idle::load(db, request.run())?;
  let waits = run
    .execution(request.execution())
    .is_some_and(|execution| execution.status() == StepStatus::Waiting(WaitReason::Message));
  let latest = db
    .invocations(run.id())
    .map_err(store)?
    .into_iter()
    .rev()
    .find(|invocation| invocation.execution == request.execution())
    .map(|invocation| invocation.id);
  let profile = run
    .execution(request.execution())
    .and_then(|execution| run.snapshot().step(execution.step()))
    .and_then(|step| match step.executor() {
      StepExecutor::Agent { profile, .. } => Some(profile.resource),
      StepExecutor::Tool { .. } => None,
    });
  let (true, Some(profile)) = (waits && latest == Some(request.invocation()), profile) else {
    db.end_waiting_question(run.id(), request.id()).map_err(store)?;
    return Err(Rejection::QuestionNotAwaited);
  };
  let body = MessageBody::new(&answers_for_agent(request.questions(), answers))
    .map_err(|error| failed("the answer says nothing", &error))?;
  let now = Timestamp::now();
  let message = Message::from_person(run.task(), Some(run.id()), Recipient::Agent(profile), body, now);
  if !db
    .answer_questions((run.id(), request.id()), answers, (by, now), Some(message.id()))
    .map_err(store)?
  {
    return Err(refusal(db, run.id(), request.id()).map_err(store)?);
  }
  db.send_message(IdempotencyKey::generate(), &message).map_err(store)?;
  tracing::info!(run = %run.id(), execution = %request.execution(), "a step's question was answered");
  Ok(run.task())
}

/// Whether a question with suggested answers that ended `execution`'s step still waits: messages written
/// meanwhile do not resume the step, they reach the agent with the answer.
pub(super) fn held_by_question(db: &ProjectDb, run: RunId, execution: StepExecutionId) -> Result<bool, StoreError> {
  Ok(db.questions(run)?.iter().any(|asked| {
    asked.waits() && asked.request.turn() == QuestionTurn::StepEnd && asked.request.execution() == execution
  }))
}

/// Ends request `id` of `run`, which no live agent holds, if it still waits: whether it did, and why its
/// answer is refused.
fn end_unheld(db: &mut ProjectDb, run: RunId, id: &str) -> Result<(bool, Rejection), StoreError> {
  if db.end_waiting_question(run, id)? {
    return Ok((true, Rejection::QuestionNotAwaited));
  }
  Ok((false, refusal(db, run, id)?))
}

/// Why an answer to request `id` of `run`, which no longer waits for this answer, is refused.
pub(super) fn refusal(db: &ProjectDb, run: RunId, id: &str) -> Result<Rejection, StoreError> {
  Ok(match db.question_request(run, id)?.map(|asked| asked.state) {
    Some(QuestionState::Answered { .. }) => Rejection::QuestionAnswered,
    _ => Rejection::QuestionNotAwaited,
  })
}

/// Tells the desktop that questions of `run` changed.
fn announce(db: &ProjectDb, notifier: &Notifier, run: RunId) {
  match orchester_core::store::RunStore::run(db, run) {
    Ok(Some(found)) => notifier.send(Event::QuestionChanged {
      run,
      task: found.task(),
    }),
    Ok(None) => tracing::warn!(%run, "an ended question names no run"),
    Err(error) => tracing::warn!(%run, %error, "the run of an ended question cannot be read"),
  }
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
    let (ended, refusal) = end_unheld(&mut self.db, self.run.id(), id)?;
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
          Ok(self.db.end_waiting_question(run, asked.request.id())? || ended)
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
