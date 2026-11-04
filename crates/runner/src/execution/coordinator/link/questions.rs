//! The questions area of the local link: a run's questions with suggested answers, read from the project database.

use std::ops::Deref;

use orchester_core::question::AskedQuestions;
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  questions::{QuestionLink, QuestionRequestById, QuestionsRequest},
};

impl<D: Deref<Target = ProjectDb>> QuestionLink for LocalLink<D> {
  fn questions(&self, request: &QuestionsRequest) -> Result<Vec<AskedQuestions>, LinkError> {
    self.0.questions(request.run).map_err(|error| read_failed(&error))
  }

  fn question(&self, request: &QuestionRequestById) -> Result<Option<AskedQuestions>, LinkError> {
    self
      .0
      .question_request(request.run, &request.id)
      .map_err(|error| read_failed(&error))
  }
}
