//! Questions with suggested answers an agent waits on in its turn (`orchester_core::question`): Codex's
//! `requestUserInput` and Claude Code's `ask_question`, which the relay holds. The request is recorded and
//! shown, the agent's time stops counting while the person answers, and the answers go to exactly the
//! request or call that asked; the agent goes on in the same turn. An answer to a request this agent does
//! not hold any more is refused. A question Orchester does not put to the person (a secret, one without
//! options) is answered at once with why, and the activity names it. A native agent's question ends its
//! step instead ([`super::worker`]'s native side records it).

use orchester_core::{
  connection::{
    app_server::PendingQuestion,
    stream::{AgentEvent, QuestionRefusal, RunnerProblem},
  },
  question::{GivenAnswer, Question, QuestionRequest, QuestionTurn, answers_for_agent},
  run::Answerer,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::StoreError;
use tokio::sync::oneshot;

use super::{
  super::{
    control::{Ack, answer},
    question::refusal,
  },
  worker::{Watch, Worker},
};

/// The request or call a waiting question answers.
pub(super) enum AwaitingAnswer {
  Codex(Box<PendingQuestion>),
  /// A call of `ask_question` the relay holds; it gets the answers as the call's result.
  Relay(oneshot::Sender<String>),
}

/// A call of `ask_question` the relay holds until the person answers.
pub(in super::super) struct AskedQuestion {
  /// Names the call within its invocation.
  pub(in super::super) call: String,
  pub(in super::super) question: Question,
  pub(in super::super) reply: oneshot::Sender<String>,
}

/// Answers to a waiting request, on their way to the worker that holds it.
pub(in super::super) struct QuestionAnswer {
  pub request: String,
  pub answers: Vec<GivenAnswer>,
  pub by: Answerer,
  pub ack: Ack,
}

impl Worker {
  pub(super) fn request_codex_question(
    &mut self,
    watch: &mut Watch,
    pending: PendingQuestion,
  ) -> Result<(), StoreError> {
    let key = pending.request_key();
    let questions = pending.questions().to_vec();
    self.ask_in_turn(watch, &key, questions, AwaitingAnswer::Codex(Box::new(pending)))
  }

  pub(super) fn request_relay_question(&mut self, watch: &mut Watch, asked: AskedQuestion) -> Result<(), StoreError> {
    let AskedQuestion { call, question, reply } = asked;
    self.ask_in_turn(watch, &call, vec![question], AwaitingAnswer::Relay(reply))
  }

  fn ask_in_turn(
    &mut self,
    watch: &mut Watch,
    key: &str,
    questions: Vec<Question>,
    awaiting: AwaitingAnswer,
  ) -> Result<(), StoreError> {
    if watch.question.is_some() {
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("{} asked another question while one waits", self.cli()),
        },
      )?;
      self.driver.end();
      return Ok(());
    }
    let invocation = &watch.invocation;
    let now = Timestamp::now();
    let request = QuestionRequest::new(
      format!("{}:{key}", invocation.id),
      (invocation.run, invocation.execution, invocation.id),
      self.agent.clone(),
      questions,
      (now, QuestionTurn::InTurn),
    )
    .expect("Codex's question requests and the relay's ask_question calls each carry a question");
    watch.db.ask_questions(&request)?;
    watch.invocation.timing.wait_for_person(now);
    // A person's wait never holds the computer's QA slot.
    self.qa.release(watch.invocation.id);
    watch.db.save_invocation(&watch.invocation)?;
    self.question_changed(watch);
    watch.question = Some((request, awaiting));
    Ok(())
  }

  /// Gives `answer` to the request this agent holds, or refuses it.
  pub(super) async fn answer_question(&mut self, watch: &mut Watch, given: QuestionAnswer) -> Result<(), StoreError> {
    let QuestionAnswer {
      request,
      answers,
      by,
      ack,
    } = given;
    let held = watch
      .question
      .as_ref()
      .is_some_and(|(waiting, _)| waiting.id() == request);
    if !held || watch.stop.is_some() {
      let refused = refusal(&watch.db, watch.invocation.run, &request)?;
      answer(ack, Err(refused));
      return Ok(());
    }
    let (waiting, awaiting) = watch.question.take().expect("the held question was matched above");
    let answers = match waiting.answer(&answers) {
      Ok(answers) => answers,
      Err(error) => {
        watch.question = Some((waiting, awaiting));
        answer(ack, Err(Rejection::InvalidAnswer { error }));
        return Ok(());
      }
    };
    let run = waiting.run();
    if !watch
      .db
      .answer_questions((run, waiting.id()), &answers, (by, Timestamp::now()), None)?
    {
      let refused = refusal(&watch.db, run, waiting.id())?;
      // The agent still waits for this call; its end interrupts what is recorded of it.
      watch.question = Some((waiting, awaiting));
      answer(ack, Err(refused));
      return Ok(());
    }
    let delivered = match awaiting {
      AwaitingAnswer::Codex(pending) => match self.driver.process() {
        Some(process) => process
          .send_line(pending.response(&answers))
          .await
          .map_err(|error| error.to_string()),
        None => Err("Codex has no process".to_owned()),
      },
      AwaitingAnswer::Relay(reply) => reply
        .send(answers_for_agent(waiting.questions(), &answers))
        .map_err(|_| "Orchester's tool relay has closed".to_owned()),
    };
    self.end_question_wait(watch)?;
    if let Err(error) = delivered {
      watch.db.undelivered_questions(run, waiting.id())?;
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("The {} session closed before the answer arrived", self.cli()),
        },
      )?;
      self.driver.end();
      answer(
        ack,
        Err(Rejection::Failed {
          detail: format!("cannot send the answer to {}: {error}", self.cli()),
        }),
      );
      return Ok(());
    }
    answer(ack, Ok(()));
    Ok(())
  }

  /// The relay closed while its `ask_question` call waited: the question can no longer be answered, and
  /// the turn fails with it.
  pub(super) fn question_closed(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    watch.question = None;
    self.interrupt_questions(watch)?;
    self.observe(
      watch,
      AgentEvent::TurnFailed {
        message: format!("{}'s tool relay closed before the question was answered", self.cli()),
      },
    )?;
    self.driver.end();
    Ok(())
  }

  /// The agent ended: a question still waiting in its turn can no longer be answered.
  pub(super) fn interrupt_questions(&self, watch: &mut Watch) -> Result<(), StoreError> {
    watch.question = None;
    if watch
      .db
      .interrupt_questions(watch.invocation.run, watch.invocation.id)?
    {
      self.end_question_wait(watch)?;
    }
    Ok(())
  }

  /// Answers a question Orchester does not put to the person with `response`, and names it in the activity.
  pub(super) async fn refuse_question(
    &self,
    watch: &mut Watch,
    response: String,
    (question, reason): (String, QuestionRefusal),
  ) -> Result<(), StoreError> {
    // A process that cannot take the answer is ending; its exit says how.
    if let Some(process) = self.driver.process()
      && let Err(error) = process.send_line(response).await
    {
      tracing::warn!(%error, "Codex App Server did not take a refused question's answer");
    }
    let problem = RunnerProblem::QuestionRefused {
      question: self.redact(&question),
      reason,
    };
    self.observe(watch, AgentEvent::RunnerProblem { problem })
  }

  fn end_question_wait(&self, watch: &mut Watch) -> Result<(), StoreError> {
    watch.invocation.timing.resume(Timestamp::now());
    self.resume_qa(watch, None);
    watch.db.save_invocation(&watch.invocation)?;
    self.question_changed(watch);
    Ok(())
  }

  fn question_changed(&self, watch: &Watch) {
    self.notifier.send(Event::QuestionChanged {
      run: watch.invocation.run,
      task: self.task,
    });
  }
}
