//! The questions area of the local link: a run's questions with suggested answers, read from the project database.
//! In the runner's own tests each request and answer crosses as JSON (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::question::AskedQuestions;
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
use crate::execution::executor::link::{
  LinkError,
  questions::{QuestionLink, QuestionRequestById, QuestionsRequest},
};

impl<D: Deref<Target = ProjectDb>> QuestionLink for LocalLink<D> {
  fn questions(&self, request: &QuestionsRequest) -> Result<Vec<AskedQuestions>, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    let answer = self.0.questions(request.run).map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }

  fn question(&self, request: &QuestionRequestById) -> Result<Option<AskedQuestions>, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    let answer = self
      .0
      .question_request(request.run, &request.id)
      .map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }
}
