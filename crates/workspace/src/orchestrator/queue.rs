//! Writing to the Orchestrator while it answers (ORC-92, `docs/design/orchestrator-toolset.md`, "Writing while it
//! answers"), as in Claude Code desktop: *Send after this answer*, the default, keeps the message queued; *Send now*
//! keeps it handed to the answer under way and asks the project's runner to give it to that turn's CLI, which takes it
//! at its next tool call; *Stop and send* keeps it to go next and stops the answer. A queued message gets its own turn
//! once its conversation's turn ended, in order, a *Stop and send* one first: the app's follower starts it
//! ([`start_next_in`]) as it starts the turn of an agent's wait. Until then the person may remove it, or take its words
//! back into the composer.

use std::path::Path;

use orchester_core::{
  id::ChatMessageId,
  orchestrator::{Queued, TurnState},
};
use orchester_store::ProjectDb;

use super::{
  send::{Outgoing, SendError, keep_as_in},
  waits::WaitTurn,
};
use crate::{
  background,
  messages::{self, Message},
  runner::{RequestError, RunRequestError, Runners},
  workspace::OpenProject,
};

/// How a message written while the Orchestrator answers goes to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhileAnswering {
  /// *Send after this answer*: its own turn starts once the answer ended, after those queued before it.
  AfterAnswer,
  /// *Send now*: the answer under way takes it at its next tool call; one it does not take is answered after it.
  Now,
  /// *Stop and send*: the answer under way stops, and its turn starts next.
  StopAndSend,
}

impl WhileAnswering {
  /// The turn the message is kept with.
  const fn kept_as(self) -> TurnState {
    match self {
      Self::AfterAnswer => TurnState::Queued(Queued::AfterAnswer),
      Self::Now => TurnState::Steering,
      Self::StopAndSend => TurnState::Queued(Queued::Next),
    }
  }
}

/// Keeps `outgoing`, written while the Orchestrator answers, as `how` says, and asks `project`'s runner what follows:
/// Send now hands it to the answer under way, Stop and send stops that answer. Nothing more is asked for a message
/// sent after the answer: the follower starts its turn. A message the runner could not be asked to hand over waits for
/// a turn of its own; a stop the runner did not confirm leaves the message to go next all the same.
pub async fn send_while_answering(
  runners: &Runners,
  project: &OpenProject,
  outgoing: &Outgoing,
  how: WhileAnswering,
) -> Result<(), SendError> {
  let (root, kept) = (project.repo_root.clone(), outgoing.clone());
  background::run(move || keep_as_in(&root, &kept, &how.kept_as()))
    .await
    .and_then(|kept| kept)
    .map_err(SendError::NotSent)?;
  runners.chat_changed(&project.repo_root);
  match how {
    WhileAnswering::AfterAnswer => Ok(()),
    WhileAnswering::Now => {
      if let Err(error) = runners.steer_chat(project, outgoing.message).await {
        tracing::info!(
          ?error,
          "the runner did not take a message sent now; it waits for the answer to end"
        );
        leave_for_own_turn(project, outgoing.message).await;
        runners.chat_changed(&project.repo_root);
      }
      Ok(())
    }
    WhileAnswering::StopAndSend => match runners.stop_chat(project, outgoing.chat).await {
      Ok(()) => Ok(()),
      Err(RunRequestError::Request(RequestError::Lost)) => {
        Err(SendError::Unconfirmed(Message::new("orchestrator.send.lost")))
      }
      Err(error) => {
        tracing::warn!(?error, "the answer was not stopped; the message goes next all the same");
        Err(SendError::Unconfirmed(Message::new("orchestrator.stop.failed")))
      }
    },
  }
}

/// A message sent now that no runner was asked to hand over waits for a turn of its own.
async fn leave_for_own_turn(project: &OpenProject, message: ChatMessageId) {
  let root = project.repo_root.clone();
  let left = background::run(move || {
    ProjectDb::open(&root)?.change_chat_turn(message, &[TurnState::Steering], &TurnState::Queued(Queued::NotTaken))
  })
  .await;
  // A message left handed over waits for its own turn once a runner attaches and holds no turn of its conversation.
  match left {
    Ok(Ok(_)) => {}
    Ok(Err(error)) => tracing::warn!(%error, "cannot leave a message sent now for a turn of its own"),
    Err(error) => tracing::warn!(?error, "cannot leave a message sent now for a turn of its own"),
  }
}

/// Starts the turn of the next queued message of the project at `root` whose conversation no turn holds, if any: its
/// conversation and the message whose turn it is. Blocks on SQLite.
pub fn start_next_in(root: &Path) -> Result<Option<WaitTurn>, Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.start_queued_chat_turn())
    .map(|started| {
      started.map(|started| WaitTurn {
        chat: started.chat,
        message: started.message,
      })
    })
    .map_err(|error| messages::store_error(&error))
}

/// Whether a queued message of the project at `root` could start its turn now. Blocks on SQLite.
pub fn next_ready_in(root: &Path) -> Result<bool, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.queued_chat_turn_ready())
    .map_err(|error| messages::store_error(&error))
}

/// Removes the queued message `message` of the project at `root`, with its files; whether it still waited. Blocks on
/// SQLite.
pub fn remove_in(root: &Path, message: ChatMessageId) -> Result<bool, Message> {
  ProjectDb::open(root)
    .and_then(|mut db| db.remove_queued_chat_message(message))
    .map_err(|error| messages::store_error(&error))
}
