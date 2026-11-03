//! Side questions about a run answered by a project's runner ([`Command::AnswerSideQuestion`]): a separate agent on
//! the Orchestrator's connection and model, whose keys only the runner reads, and *Stop*.

use orchester_core::{
  id::SideQuestionId,
  run::side_question::{SideInput, SideOutcome},
};
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

impl Runners {
  /// Asks `project`'s runner, starting it if needed, to answer side question `question` from `input`.
  pub async fn answer_side_question_in(
    &self,
    project: &OpenProject,
    question: SideQuestionId,
    input: SideInput,
  ) -> Result<SideOutcome, RunRequestError> {
    let command = Command::AnswerSideQuestion {
      question,
      input: Box::new(input),
    };
    match self.ask_project(project, command).await? {
      Reply::SideAnswered { outcome, .. } => Ok(outcome),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to AnswerSideQuestion");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks `project`'s runner to stop the side agent answering `question`.
  pub async fn stop_side_question_in(
    &self,
    project: &OpenProject,
    question: SideQuestionId,
  ) -> Result<(), RunRequestError> {
    match self
      .ask_project(project, Command::StopSideQuestion { question })
      .await?
    {
      Reply::SideStopped { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to StopSideQuestion");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
