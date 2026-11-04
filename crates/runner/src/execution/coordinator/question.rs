//! Answers to agents' questions with suggested answers as the scheduler takes them
//! (`crate::execution::executor::question`): from the app's card and from Orchester's MCP server alike, checked
//! against the questions first. One a step's end asked is recorded here with the person's message that carries it;
//! one asked in a turn goes to the task executing the run, and ends refused when no task does.

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId, TaskId},
  question::{Answer, AskedQuestions, GivenAnswer, QuestionRequest, QuestionState, QuestionTurn, answers_for_agent},
  run::{Answerer, StepExecutor, StepStatus, WaitReason},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};

use super::{Notifier, Scheduler, failed, idle, sink::LocalSink};
use crate::execution::executor::{
  control::Control,
  question::{end_unheld, refusal},
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
    let mut sink = LocalSink::new(&self.inner.project_root);
    match end_unheld((db, &mut sink), run, id) {
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
  let message = match by {
    Answerer::Person => message,
    Answerer::Automation => message
      .sent_by_automation()
      .expect("the new message is on the person's side"),
  };
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
