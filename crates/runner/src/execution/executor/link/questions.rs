//! The questions area of the coordinator link: the questions with suggested answers a run's agents asked, with where
//! each stands, which the executor reads to route an answer, to tell whether a question still holds its step, and for
//! `get_task`.

use orchester_core::{id::RunId, question::AskedQuestions};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about a run's questions.
pub trait QuestionLink {
  /// The requests of the run `request` names, oldest first, with where each stands.
  fn questions(&self, request: &QuestionsRequest) -> Result<Vec<AskedQuestions>, LinkError>;

  /// The request `request` names; `None` when the run has no such request.
  fn question(&self, request: &QuestionRequestById) -> Result<Option<AskedQuestions>, LinkError>;
}

/// A run's questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionsRequest {
  pub run: RunId,
}

/// One request of a run, by its id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionRequestById {
  pub run: RunId,
  pub id: String,
}

/// The questions of `run`, asked through `link`.
pub fn questions(link: &(impl QuestionLink + ?Sized), run: RunId) -> Result<Vec<AskedQuestions>, LinkError> {
  link.questions(&QuestionsRequest { run })
}

/// Request `id` of `run`, asked through `link`.
pub fn question(
  link: &(impl QuestionLink + ?Sized),
  run: RunId,
  id: &str,
) -> Result<Option<AskedQuestions>, LinkError> {
  link.question(&QuestionRequestById { run, id: id.to_owned() })
}
