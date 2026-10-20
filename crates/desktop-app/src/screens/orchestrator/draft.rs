//! The composer's draft of each conversation for as long as the app runs, beyond the screen: leaving the
//! Orchestrator and coming back, or switching conversations, finds each one's own. A new conversation's draft is the
//! project's until its first message is kept; whatever the person wrote meanwhile then goes with the conversation.
//!
//! Each draft also keeps the message it was last sent as until that message is kept, beyond the screen too: a send
//! still under way when the person leaves ends with the screen, and sending the same words again then keeps the
//! message once.

use orchester_core::id::{ChatId, ChatMessageId, ProjectId};
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
  sent: Option<Outgoing>,
}

impl Entry for Draft {
  fn is_empty(&self) -> bool {
    self.text.is_empty() && self.sent.is_none()
  }
}

static DRAFTS: ByKey<(ProjectId, Conversation), Draft> = ByKey::new();

pub(super) fn draft(project: ProjectId, of: Conversation) -> String {
  DRAFTS.read((project, of), |draft| draft.text.clone())
}

pub(super) fn set(project: ProjectId, of: Conversation, text: &str) {
  DRAFTS.change((project, of), |draft| text.clone_into(&mut draft.text));
}

/// The message the draft of `of` goes as: the one it was last sent as when its words did not change since, so that
/// one is kept once, or a new one to `chat`.
pub(super) fn sending(project: ProjectId, of: Conversation, (chat, text): (ChatId, String)) -> Outgoing {
  DRAFTS.change((project, of), |draft| {
    let outgoing = match draft.sent.take() {
      Some(sent) if sent.text == text => sent,
      _ => Outgoing {
        chat,
        message: ChatMessageId::generate(),
        text,
      },
    };
    draft.sent = Some(outgoing.clone());
    outgoing
  })
}

/// Message `message` of conversation `chat`, a draft's, was sent and kept: the draft it came from (`chat`'s, or the
/// new conversation's) forgets it, and empties unless it holds other words by now. Which draft it came from; `None`
/// when no draft was sent as it.
pub(super) fn sent(project: ProjectId, (chat, message): (ChatId, ChatMessageId)) -> Option<Conversation> {
  [Conversation::Chat(chat), Conversation::New].into_iter().find(|&of| {
    DRAFTS.change((project, of), |draft| {
      let Some(sent) = draft.sent.take_if(|sent| sent.message == message) else {
        return false;
      };
      if draft.text == sent.text {
        draft.text.clear();
      }
      true
    })
  })
}

/// The new conversation's first message made it conversation `chat`: what its draft holds is `chat`'s now.
pub(super) fn started(project: ProjectId, chat: ChatId) {
  let left = DRAFTS.change((project, Conversation::New), std::mem::take);
  DRAFTS.change((project, Conversation::Chat(chat)), |draft| *draft = left);
}
