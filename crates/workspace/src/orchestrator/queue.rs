//! Writing to the Orchestrator while it answers (ORC-92, `docs/design/orchestrator-toolset.md`, "Writing while it
//! answers"), as in Claude Code desktop: *Send after this answer*, the default, keeps the message queued; *Send now*
//! keeps it handed to the answer under way and asks the project's runner to give it to that turn's CLI, which takes it
//! at its next tool call; *Stop and send* keeps it to go next and stops the answer. A queued message gets its own turn
//! once its conversation's turn ended, in order, a *Stop and send* one first: the app's follower starts it
//! ([`start_next_in`]) as it starts the turn of an agent's wait. Until then the person may remove it, or take its words
//! and files back into the composer ([`take_back_in`]).

use std::path::Path;

use orchester_core::{
  attachment::{Attachment, NewAttachment, input::AttachedFile, is_names_only},
  id::ChatMessageId,
  orchestrator::{Queued, TurnState},
};
use orchester_store::{AttachmentOwner, ProjectDb, StoreError};

use super::{
  send::{Outgoing, SendError, keep_as_in},
  waits::WaitTurn,
};
use crate::{
  attachments::copy_unreadable,
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

/// Why an *Edit* or *Remove* of a queued message changed nothing: its turn had started meanwhile.
pub const fn already_started() -> Message {
  Message::new("orchestrator.queued.started")
}

/// A queued message taken back into the composer to be reworded: its words, none for a message of files only, and its
/// files as a draft holds them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TakenBack {
  pub words: String,
  pub attachments: Vec<NewAttachment>,
}

/// Takes the queued message `message` of the project at `root`, which says `text`, back to be reworded: its files are
/// read from the project's copies, then it is removed with them, so it is never sent as well. `None` when its turn
/// started meanwhile; refused, leaving it queued, when a copy cannot be read. Blocks on SQLite and the file system.
pub fn take_back_in(root: &Path, message: ChatMessageId, text: &str) -> Result<Option<TakenBack>, Message> {
  let store = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(store)?;
  let files = db
    .attached_files(AttachmentOwner::ChatMessage(message))
    .map_err(store)?;
  let attachments = files.iter().map(read_back).collect::<Result<Vec<_>, _>>()?;
  if !db.remove_queued_chat_message(message).map_err(store)? {
    return Ok(None);
  }
  let kept: Vec<Attachment> = files.into_iter().map(|file| file.attachment).collect();
  let words = if is_names_only(text, &kept) {
    String::new()
  } else {
    text.to_owned()
  };
  Ok(Some(TakenBack { words, attachments }))
}

/// The project's copy of a queued message's file, as a draft holds it again.
fn read_back(file: &AttachedFile) -> Result<NewAttachment, Message> {
  let name = file.attachment.name();
  let content = file.read().map_err(|error| {
    tracing::warn!(%error, path = %file.path.display(), "a queued message's file cannot be read back");
    copy_unreadable(name.as_str())
  })?;
  NewAttachment::new(name.clone(), content).map_err(|refusal| {
    tracing::warn!(?refusal, "a queued message's file is no longer taken");
    copy_unreadable(name.as_str())
  })
}
