//! The messages area of the coordinator link: the messages of a run's task, the person's and the agents', and the
//! files people attached to one, with their content. A file is at most 20 MB and a message's files 50 MB together
//! (`orchester_core::attachment::limits`), so they travel in the answer: an executor needs no access to the project's
//! own copies. Where it keeps the copies it gives its agents is its own ([`super::super::attached`]).

use std::fmt;

use orchester_core::{
  attachment::Attachment,
  conversation::Message,
  id::{MessageId, TaskId},
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about a task's conversation.
pub trait MessageLink {
  /// The messages of the task `request` names, in the order they were written.
  fn messages(&self, request: &MessagesRequest) -> Result<Vec<Message>, LinkError>;

  /// The files people attached to the message `request` names, in the order they attached them, with their content.
  fn attached(&self, request: &AttachedRequest) -> Result<Vec<AttachedContent>, LinkError>;
}

/// A task's conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagesRequest {
  pub task: TaskId,
}

/// The files attached to a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedRequest {
  pub message: MessageId,
}

/// A file attached to a message, and its content: why it could not be read when the project's copy could not be,
/// which fails only the agent input it goes into, as before. Like the attachment, `Debug` shows what it is, never its
/// content.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedContent {
  pub attachment: Attachment,
  pub content: Result<Vec<u8>, String>,
}

impl fmt::Debug for AttachedContent {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let content = self.content.as_ref().map(Vec::len);
    f.debug_struct("AttachedContent")
      .field("attachment", &self.attachment)
      .field("bytes", &content)
      .finish()
  }
}

/// The messages of `task`, asked through `link`.
pub fn messages(link: &(impl MessageLink + ?Sized), task: TaskId) -> Result<Vec<Message>, LinkError> {
  link.messages(&MessagesRequest { task })
}

/// The files attached to `message`, with their content, asked through `link`.
pub fn attached(link: &(impl MessageLink + ?Sized), message: MessageId) -> Result<Vec<AttachedContent>, LinkError> {
  link.attached(&AttachedRequest { message })
}
