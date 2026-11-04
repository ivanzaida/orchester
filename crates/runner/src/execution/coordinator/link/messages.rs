//! The messages area of the local link: a task's messages, and the files attached to one with their content, read
//! from the project database and the project's copies.

use std::ops::Deref;

use orchester_core::{conversation::Message, store::ConversationStore};
use orchester_store::{AttachmentOwner, ProjectDb};

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  messages::{AttachedContent, AttachedRequest, MessageLink, MessagesRequest},
};

impl<D: Deref<Target = ProjectDb>> MessageLink for LocalLink<D> {
  fn messages(&self, request: &MessagesRequest) -> Result<Vec<Message>, LinkError> {
    self.0.messages(request.task).map_err(|error| read_failed(&error))
  }

  fn attached(&self, request: &AttachedRequest) -> Result<Vec<AttachedContent>, LinkError> {
    let files = self
      .0
      .attached_files(AttachmentOwner::Message(request.message))
      .map_err(|error| read_failed(&error))?;
    Ok(
      files
        .into_iter()
        .map(|file| AttachedContent {
          content: file.read().map_err(|error| format!("{}: {error}", file.path.display())),
          attachment: file.attachment,
        })
        .collect(),
    )
  }
}
