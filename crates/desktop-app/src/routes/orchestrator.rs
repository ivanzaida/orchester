use orchester_core::id::ChatId;

use super::{
  RouteError,
  query::{Params, with_query},
};

const CHAT: &str = "chat";
const NEW_CHAT: &str = "new";

/// Which conversation the Orchestrator shows: the one with the latest message, the one a person chose in
/// History, or a new one. The latest is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrchestratorChat {
  #[default]
  Latest,
  New,
  Chat(ChatId),
}

impl OrchestratorChat {
  pub(super) fn path(self, orchestrator: String) -> String {
    let chat = match self {
      Self::Latest => None,
      Self::New => Some(NEW_CHAT.to_owned()),
      Self::Chat(id) => Some(id.to_string()),
    };
    with_query(orchestrator, &[(CHAT, chat.as_deref())])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    match params.take(CHAT)?.as_deref() {
      None => Ok(Self::Latest),
      Some(NEW_CHAT) => Ok(Self::New),
      Some(id) => id.parse().map(Self::Chat).map_err(|_| RouteError::InvalidParam {
        param: CHAT.to_owned(),
        value: id.to_owned(),
      }),
    }
  }
}
