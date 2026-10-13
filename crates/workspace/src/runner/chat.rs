//! The Orchestrator chat's requests to a project's runner (`orchester_core::orchestrator`): answer a message the
//! desktop kept, stop the answer under way, and the counter the chat reads its conversations again by.

use std::path::Path;

use orchester_core::id::{ChatId, ChatMessageId};
use orchester_ipc::{ChatTools, Command, Reply};
use tokio::sync::watch;

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

impl Runners {
  /// Starts `project`'s runner if needed and waits until it attached: a runner that attaches ends the turns still
  /// thinking as interrupted, so a message is kept only once its runner is there to answer it.
  pub async fn chat_runner(&self, project: &OpenProject) -> Result<(), RequestError> {
    let root = project.repo_root.as_path();
    self.ensure(root, project.project.name().as_str());
    self.attached(root).await
  }

  /// Asks `project`'s runner to answer the person's message `message` with the domain tools at `tools`, starting the
  /// runner if needed. `Ok` once the turn is under way, or was already, or ended.
  pub async fn answer_in_chat(
    &self,
    project: &OpenProject,
    message: ChatMessageId,
    tools: ChatTools,
  ) -> Result<(), RunRequestError> {
    match self
      .ask_project(project, Command::AnswerInChat { message, tools })
      .await?
    {
      Reply::ChatAnswering { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to AnswerInChat");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Asks `project`'s runner to stop the answer under way in `chat`.
  pub async fn stop_chat(&self, project: &OpenProject, chat: ChatId) -> Result<(), RunRequestError> {
    match self
      .ask(Some(project.repo_root.as_path()), Command::StopChat { chat })
      .await?
    {
      Reply::ChatStopped { .. } => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to StopChat");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Changes whenever a conversation of project `root`'s Orchestrator chat changed.
  pub fn chats_of(&self, root: &Path) -> watch::Receiver<u64> {
    self.inner.shared.counters(root).chats.subscribe()
  }

  /// Tells the readers of [`Self::chats_of`] that the desktop changed a conversation of project `root`.
  pub fn chat_changed(&self, root: &Path) {
    self
      .inner
      .shared
      .counters(root)
      .chats
      .send_modify(|revision| *revision += 1);
  }
}
