//! Sending a person's message to the Orchestrator and stopping its answer (`Orchestrator · states`, `kDOn9`). A
//! message is kept in the project's database before the runner is asked for its turn, under the id it got when it
//! was first sent, so sending it again keeps it once. When the runner never took it, it is forgotten and its draft
//! stays with the person; when the runner's answer was lost, it stays: the runner may be answering it, and sending it
//! again asks for the same turn.

use std::path::Path;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::Chat,
  time::Timestamp,
};
use orchester_ipc::{ChatTools, Rejection};
use orchester_store::ProjectDb;

use super::settings::OrchestratorSetup;
use crate::{
  background,
  messages::{self, Message},
  runner::{RequestError, RunRequestError, Runners},
  workspace::OpenProject,
};

/// A message the person sends: the conversation it goes to (a new one gets its id here too), its own id, given once
/// for the draft, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing {
  pub chat: ChatId,
  pub message: ChatMessageId,
  pub text: String,
}

/// Why a message did not reach the Orchester's agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendError {
  /// Nothing was kept or asked: the draft is all there is.
  NotSent(Message),
  /// The message is kept, and whether the runner took it is not known.
  Unconfirmed(Message),
}

/// Keeps `outgoing` and asks `project`'s runner to answer it with the domain tools at `tools`.
pub async fn send(
  runners: &Runners,
  project: &OpenProject,
  outgoing: &Outgoing,
  tools: ChatTools,
) -> Result<(), SendError> {
  let root = project.repo_root.clone();
  let kept = outgoing.clone();
  background::run(move || keep_in(&root, &kept))
    .await
    .and_then(|kept| kept)
    .map_err(SendError::NotSent)?;
  runners.chat_changed(&project.repo_root);
  let asked = runners.answer_in_chat(project, outgoing.message, tools).await;
  let Err(error) = asked else {
    return Ok(());
  };
  if matches!(error, RunRequestError::Request(RequestError::Lost)) {
    return Err(SendError::Unconfirmed(Message::new("orchestrator.send.lost")));
  }
  let (root, message) = (project.repo_root.clone(), outgoing.message);
  let forgotten =
    background::run(move || ProjectDb::open(&root).and_then(|mut db| db.forget_chat_message(message))).await;
  // A message left kept shows as thinking until a runner starts and ends it as interrupted; the draft stays.
  match forgotten {
    Ok(Ok(())) => {}
    Ok(Err(error)) => tracing::warn!(%error, "cannot forget a chat message the runner did not take"),
    Err(error) => tracing::warn!(?error, "cannot forget a chat message the runner did not take"),
  }
  runners.chat_changed(&project.repo_root);
  Err(SendError::NotSent(not_sent(&error)))
}

/// Asks `project`'s runner to stop the answer under way in `chat`.
pub async fn stop(runners: &Runners, project: &OpenProject, chat: ChatId) -> Result<(), Message> {
  runners
    .stop_chat(project, chat)
    .await
    .map_err(|_| Message::new("orchestrator.stop.failed"))
}

/// Keeps `outgoing` in the project at `root`, starting its conversation when it is new; refused while the
/// Orchestrator has nothing it can run on. Blocks on SQLite.
pub fn keep_in(root: &Path, outgoing: &Outgoing) -> Result<(), Message> {
  let text = MessageBody::new(&outgoing.text).map_err(|_| Message::new("orchestrator.send.blank"))?;
  if !OrchestratorSetup::load_in(root)?.readiness().is_ready() {
    return Err(Message::new("orchestrator.send.notReady"));
  }
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let now = Timestamp::now();
  let chat = db
    .chat(outgoing.chat)
    .map_err(|error| messages::store_error(&error))?
    .unwrap_or_else(|| Chat::start(outgoing.chat, &text, now));
  db.keep_chat_message(&chat, (outgoing.message, &text), now)
    .map(drop)
    .map_err(|error| messages::store_error(&error))
}

/// What the person reads when the runner did not take a message.
fn not_sent(error: &RunRequestError) -> Message {
  match error {
    RunRequestError::Request(RequestError::NotConnected) => Message::new("orchestrator.send.runnerUnavailable"),
    RunRequestError::Request(RequestError::Lost) => Message::new("orchestrator.send.lost"),
    RunRequestError::Rejected(Rejection::ChatBusy) => Message::new("orchestrator.send.busy"),
    RunRequestError::Rejected(Rejection::NoOrchestrator) => Message::new("orchestrator.send.notReady"),
    RunRequestError::Rejected(Rejection::ShuttingDown) => Message::new("orchestrator.send.shuttingDown"),
    RunRequestError::Rejected(_) => Message::new("orchestrator.send.failed"),
  }
}
