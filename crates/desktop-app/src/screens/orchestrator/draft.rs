//! The composer's draft of each conversation for as long as the app runs, beyond the screen: leaving the
//! Orchestrator and coming back, or switching conversations, finds each one's own, its words and its attached files.
//! A new conversation's draft is the project's until its first message is kept; whatever the person wrote meanwhile
//! then goes with the conversation.
//!
//! Each draft also keeps the message it was last sent as until that message is kept, beyond the screen too: a send
//! still under way when the person leaves ends with the screen, and sending the same words and files again then
//! keeps the message once.

use orchester_core::{
  attachment::NewAttachment,
  id::{ChatId, ChatMessageId, ProjectId},
  run::Answerer,
};
use orchester_workspace::orchestrator::send::Outgoing;

use crate::screens::kept::{ByKey, Entry};

/// Whose draft the composer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Conversation {
  New,
  Chat(ChatId),
}

impl lurq::app::component::DevtoolsInspectable for Conversation {}

impl From<Option<ChatId>> for Conversation {
  fn from(chat: Option<ChatId>) -> Self {
    chat.map_or(Self::New, Self::Chat)
  }
}

/// One conversation's draft, and the message it was last sent as while that one is not known to be kept.
#[derive(Default)]
struct Draft {
  text: String,
  attachments: Vec<NewAttachment>,
  sent: Option<Outgoing>,
}

impl Entry for Draft {
  fn is_empty(&self) -> bool {
    self.text.is_empty() && self.attachments.is_empty() && self.sent.is_none()
  }
}

static DRAFTS: ByKey<(ProjectId, Conversation), Draft> = ByKey::new();

pub(super) fn draft(project: ProjectId, of: Conversation) -> String {
  DRAFTS.read((project, of), |draft| draft.text.clone())
}

pub(super) fn set(project: ProjectId, of: Conversation, text: &str) {
  DRAFTS.change((project, of), |draft| text.clone_into(&mut draft.text));
}

/// The files the draft of `of` holds.
pub(super) fn attachments(project: ProjectId, of: Conversation) -> Vec<NewAttachment> {
  DRAFTS.read((project, of), |draft| draft.attachments.clone())
}

pub(super) fn set_attachments(project: ProjectId, of: Conversation, attachments: &[NewAttachment]) {
  DRAFTS.change((project, of), |draft| attachments.clone_into(&mut draft.attachments));
}

/// The message the draft of `of` goes as, sent by `sent_by`: the one it was last sent as when its words and files did
/// not change since, so that one is kept once, or a new one to `chat`.
pub(super) fn sending(
  project: ProjectId,
  of: Conversation,
  (chat, text, attachments): (ChatId, String, Vec<NewAttachment>),
  sent_by: Answerer,
) -> Outgoing {
  DRAFTS.change((project, of), |draft| {
    let outgoing = match draft.sent.take() {
      Some(sent) if sent.text == text && sent.attachments == attachments => Outgoing { sent_by, ..sent },
      _ => Outgoing {
        chat,
        message: ChatMessageId::generate(),
        text,
        attachments,
        sent_by,
      },
    };
    draft.sent = Some(outgoing.clone());
    outgoing
  })
}

/// Message `message` of conversation `chat`, a draft's, was sent and kept: the draft it came from (`chat`'s, or the
/// new conversation's) forgets it, and empties unless it holds other words and files by now. Which draft it came
/// from; `None` when no draft was sent as it.
pub(super) fn sent(project: ProjectId, (chat, message): (ChatId, ChatMessageId)) -> Option<Conversation> {
  [Conversation::Chat(chat), Conversation::New].into_iter().find(|&of| {
    DRAFTS.change((project, of), |draft| {
      let Some(sent) = draft.sent.take_if(|sent| sent.message == message) else {
        return false;
      };
      if draft.text == sent.text {
        draft.text.clear();
      }
      draft.attachments.retain(|held| !sent.attachments.contains(held));
      true
    })
  })
}

/// The new conversation's first message made it conversation `chat`: what its draft holds is `chat`'s now.
pub(super) fn started(project: ProjectId, chat: ChatId) {
  let left = DRAFTS.change((project, Conversation::New), std::mem::take);
  DRAFTS.change((project, Conversation::Chat(chat)), |draft| *draft = left);
}
