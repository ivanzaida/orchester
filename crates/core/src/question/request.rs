//! One time an agent asked: the questions of one request (Codex may ask several at once, the others ask
//! one), who asked in which run, step execution and invocation, and where it stands.

use thiserror::Error;

use super::{Answer, AnswerError, GivenAnswer, Question};
use crate::{
  id::{InvocationId, MessageId, RunId, StepExecutionId},
  run::Answerer,
  time::Timestamp,
};

/// How the asking agent waits for the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuestionTurn {
  /// Its turn waits: the answer goes back to the call or request that asked (Claude Code's
  /// `ask_question`, Codex's `requestUserInput`), and the agent goes on in the same turn.
  InTurn,
  /// Its step ended with the question and waits for a message (a native agent's `ask_question`): the
  /// answer resumes its session.
  StepEnd,
}

impl QuestionTurn {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::InTurn => "in_turn",
      Self::StepEnd => "step_end",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::InTurn, Self::StepEnd]
      .into_iter()
      .find(|turn| turn.as_str() == value)
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a request asks at least one question")]
pub struct NoQuestions;

/// The questions of one request and who asked them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionRequest {
  id: String,
  run: RunId,
  execution: StepExecutionId,
  invocation: InvocationId,
  agent: String,
  questions: Vec<Question>,
  asked_at: Timestamp,
  turn: QuestionTurn,
}

impl QuestionRequest {
  /// Request `id` (unique in its run) of `invocation`'s agent `agent`, asking `questions` at `asked_at`.
  pub fn new(
    id: String,
    (run, execution, invocation): (RunId, StepExecutionId, InvocationId),
    agent: String,
    questions: Vec<Question>,
    (asked_at, turn): (Timestamp, QuestionTurn),
  ) -> Result<Self, NoQuestions> {
    if questions.is_empty() {
      return Err(NoQuestions);
    }
    Ok(Self {
      id,
      run,
      execution,
      invocation,
      agent,
      questions,
      asked_at,
      turn,
    })
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  pub const fn execution(&self) -> StepExecutionId {
    self.execution
  }

  pub const fn invocation(&self) -> InvocationId {
    self.invocation
  }

  /// The asking agent's role.
  pub fn agent(&self) -> &str {
    &self.agent
  }

  pub fn questions(&self) -> &[Question] {
    &self.questions
  }

  pub const fn asked_at(&self) -> Timestamp {
    self.asked_at
  }

  pub const fn turn(&self) -> QuestionTurn {
    self.turn
  }

  /// The answers to every question, in the request's order, each as its question takes it.
  pub fn answer(&self, given: &[GivenAnswer]) -> Result<Vec<Answer>, AnswerError> {
    if given.len() != self.questions.len() {
      return Err(AnswerError::AnswerCount {
        expected: self.questions.len(),
        given: given.len(),
      });
    }
    self
      .questions
      .iter()
      .zip(given)
      .map(|(question, given)| question.answer(&given.choices, given.text.as_deref()))
      .collect()
  }
}

/// Where a request stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestionState {
  Waiting,
  /// `by` answered each question at `at`.
  Answered {
    answers: Vec<Answer>,
    by: Answerer,
    at: Timestamp,
  },
  /// The agent ended before an answer could reach it, at `at`.
  Interrupted {
    at: Timestamp,
  },
}

/// A request as stored, with where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskedQuestions {
  pub request: QuestionRequest,
  pub state: QuestionState,
  /// The person's message the answers reached a step-end question's agent in; the card shows them, so
  /// the message is not shown again.
  pub message: Option<MessageId>,
}

impl AskedQuestions {
  pub const fn waits(&self) -> bool {
    matches!(self.state, QuestionState::Waiting)
  }
}
