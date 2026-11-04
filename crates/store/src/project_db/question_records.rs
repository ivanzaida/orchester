//! What the runner records of agents' questions with suggested answers, as values that can be sent
//! (`docs/architecture/remote-runners.md`, rule 2): a request asked, then answered or interrupted. Each is the change
//! [`ProjectDb::write_question`] applies with the statement its direct write uses, so a request is still answered
//! once and ends once.

use orchester_core::{
  id::{InvocationId, MessageId, RunId, StepExecutionId},
  question::{Answer, GivenAnswer, Question, QuestionRequest, QuestionTurn},
  run::Answerer,
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

/// A change to the requests of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionRecord {
  run: RunId,
  change: QuestionChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum QuestionChange {
  Asked(Asked),
  Answered {
    id: String,
    answers: Vec<GivenAnswer>,
    by: Answerer,
    at: Timestamp,
    message: Option<MessageId>,
  },
  Interrupted {
    invocation: InvocationId,
  },
  Ended {
    id: String,
  },
  Undelivered {
    id: String,
  },
}

/// A request as `questions` stores it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Asked {
  id: String,
  execution: StepExecutionId,
  invocation: InvocationId,
  agent: String,
  questions: Vec<Question>,
  turn: String,
  asked_at: Timestamp,
}

impl QuestionRecord {
  /// `request`, waiting for its answer ([`ProjectDb::ask_questions`]).
  pub fn asked(request: &QuestionRequest) -> Self {
    let asked = Asked {
      id: request.id().to_owned(),
      execution: request.execution(),
      invocation: request.invocation(),
      agent: request.agent().to_owned(),
      questions: request.questions().to_vec(),
      turn: request.turn().as_str().to_owned(),
      asked_at: request.asked_at(),
    };
    Self {
      run: request.run(),
      change: QuestionChange::Asked(asked),
    }
  }

  /// `answers` to waiting request `id` of `run`, given `by` at `at`, with the person's message that carries them
  /// ([`ProjectDb::answer_questions`]).
  pub fn answered(
    (run, id): (RunId, &str),
    answers: &[Answer],
    (by, at): (Answerer, Timestamp),
    message: Option<MessageId>,
  ) -> Self {
    let change = QuestionChange::Answered {
      id: id.to_owned(),
      answers: answers.iter().map(GivenAnswer::from).collect(),
      by,
      at,
      message,
    };
    Self { run, change }
  }

  /// The requests of `invocation` in `run` still waiting in its turn, interrupted ([`ProjectDb::interrupt_questions`]).
  pub const fn interrupted(run: RunId, invocation: InvocationId) -> Self {
    Self {
      run,
      change: QuestionChange::Interrupted { invocation },
    }
  }

  /// Request `id` of `run`, ended if it still waits ([`ProjectDb::end_waiting_question`]).
  pub fn ended(run: RunId, id: &str) -> Self {
    let id = id.to_owned();
    Self {
      run,
      change: QuestionChange::Ended { id },
    }
  }

  /// Request `id` of `run`, answered, whose answer never reached its agent ([`ProjectDb::undelivered_questions`]).
  pub fn undelivered(run: RunId, id: &str) -> Self {
    let id = id.to_owned();
    Self {
      run,
      change: QuestionChange::Undelivered { id },
    }
  }
}

impl ProjectDb {
  /// Applies `record` as its direct write does: whether it changed the request it names, which an answer to or an
  /// end of a request that no longer waits does not. Asking always does; a request asked twice is refused.
  pub fn write_question(&mut self, record: &QuestionRecord) -> Result<bool, StoreError> {
    let run = record.run;
    match &record.change {
      QuestionChange::Asked(asked) => self.ask_questions(&asked.request(run)?).map(|()| true),
      QuestionChange::Answered {
        id,
        answers,
        by,
        at,
        message,
      } => self.answer_given((run, id), answers, (*by, *at), *message),
      QuestionChange::Interrupted { invocation } => self.interrupt_questions(run, *invocation),
      QuestionChange::Ended { id } => self.end_waiting_question(run, id),
      QuestionChange::Undelivered { id } => self.undelivered_questions(run, id).map(|()| true),
    }
  }
}

impl Asked {
  fn request(&self, run: RunId) -> Result<QuestionRequest, StoreError> {
    let turn = QuestionTurn::parse(&self.turn).ok_or_else(|| corrupt("question turn", &self.turn))?;
    QuestionRequest::new(
      self.id.clone(),
      (run, self.execution, self.invocation),
      self.agent.clone(),
      self.questions.clone(),
      (self.asked_at, turn),
    )
    .map_err(|error| corrupt("questions", error))
  }
}
