//! The composer's draft of each conversation for as long as the app runs, beyond the screen: leaving the
//! Orchestrator and coming back, or switching conversations, finds each one's own. A new conversation's draft is the
//! project's until its first message is kept; whatever the person wrote meanwhile then goes with the conversation.

use orchester_core::id::{ChatId, ProjectId};

use crate::screens::kept::ByKey;

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

static DRAFTS: ByKey<(ProjectId, Conversation), String> = ByKey::new();

pub(super) fn draft(project: ProjectId, of: Conversation) -> String {
  DRAFTS.read((project, of), Clone::clone)
}

pub(super) fn set(project: ProjectId, of: Conversation, text: &str) {
  DRAFTS.change((project, of), |draft| text.clone_into(draft));
}

/// `text`, the draft of `of`, was sent and kept: the draft empties unless it holds other words by now. Whether it
/// emptied.
pub(super) fn sent(project: ProjectId, of: Conversation, text: &str) -> bool {
  DRAFTS.change((project, of), |draft| {
    let unchanged = draft == text;
    if unchanged {
      draft.clear();
    }
    unchanged
  })
}

/// The new conversation's first message made it conversation `chat`: what its draft holds is `chat`'s now.
pub(super) fn started(project: ProjectId, chat: ChatId) {
  let left = DRAFTS.change((project, Conversation::New), std::mem::take);
  set(project, Conversation::Chat(chat), &left);
}
