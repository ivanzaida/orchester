//! Questions as they are read back from their serialized form (the coordinator link sends a run's questions to its
//! executor): a request is checked as [`QuestionRequest::new`] checks one, and answers against the request's questions,
//! as a stored request is read.

use serde::Deserialize;
use thiserror::Error;

use super::{
  AnswerError, AskedQuestions, GivenAnswer, NoQuestions, Question, QuestionRequest, QuestionState, QuestionTurn,
};
use crate::{
  id::{InvocationId, MessageId, RunId, StepExecutionId},
  orchestrator::FromOrchestrator,
  run::Answerer,
  time::Timestamp,
};

/// Why serialized questions are refused: they break a rule stored ones keep.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WireQuestionsError {
  #[error(transparent)]
  NoQuestions(#[from] NoQuestions),
  #[error("the answers do not answer the questions: {0}")]
  Answers(#[from] AnswerError),
  #[error("only an answered request names the Orchestrator chat that answered it")]
  OrchestratorUnanswered,
}

/// A request's fields as they were serialized, before they are checked.
#[derive(Deserialize)]
pub(super) struct RequestFields {
  id: String,
  run: RunId,
  execution: StepExecutionId,
  invocation: InvocationId,
  agent: String,
  questions: Vec<Question>,
  asked_at: Timestamp,
  turn: QuestionTurn,
}

impl TryFrom<RequestFields> for QuestionRequest {
  type Error = NoQuestions;

  fn try_from(fields: RequestFields) -> Result<Self, NoQuestions> {
    Self::new(
      fields.id,
      (fields.run, fields.execution, fields.invocation),
      fields.agent,
      fields.questions,
      (fields.asked_at, fields.turn),
    )
  }
}

/// Where a request stands, as serialized: its answers as given, before they are checked against its questions.
#[derive(Deserialize)]
enum StateFields {
  Waiting,
  Answered {
    answers: Vec<GivenAnswer>,
    by: Answerer,
    at: Timestamp,
  },
  Interrupted {
    at: Timestamp,
  },
}

/// A stored request's fields as they were serialized, before they are checked.
#[derive(Deserialize)]
pub(super) struct AskedFields {
  request: QuestionRequest,
  state: StateFields,
  message: Option<MessageId>,
  orchestrator: Option<FromOrchestrator>,
}

impl TryFrom<AskedFields> for AskedQuestions {
  type Error = WireQuestionsError;

  fn try_from(fields: AskedFields) -> Result<Self, WireQuestionsError> {
    let state = match fields.state {
      StateFields::Waiting => QuestionState::Waiting,
      StateFields::Interrupted { at } => QuestionState::Interrupted { at },
      StateFields::Answered { answers, by, at } => QuestionState::Answered {
        answers: fields.request.answer(&answers)?,
        by,
        at,
      },
    };
    if fields.orchestrator.is_some() && !matches!(state, QuestionState::Answered { .. }) {
      return Err(WireQuestionsError::OrchestratorUnanswered);
    }
    Ok(Self {
      request: fields.request,
      state,
      message: fields.message,
      orchestrator: fields.orchestrator,
    })
  }
}
