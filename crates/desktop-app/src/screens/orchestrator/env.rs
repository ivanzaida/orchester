//! What the turns' rows need from the screen ([`super::timeline::TurnEnv`]): the conversation's scope and what the
//! person can do about its newest turn and the messages that wait for it.

use std::sync::Arc;

use orchester_core::id::ChatId;
use orchester_workspace::orchestrator::tools::ChatScope;

use super::{View, send::ChangeQueued, timeline::TurnEnv};
use crate::{project::ShownProject, ui::chat::names::ProjectNames};

/// The rows' environment for conversation `chat` of `project`, on `connection` (its name) when the Orchestrator has
/// one.
pub(super) fn env(
  view: &View,
  project: &ShownProject,
  (chat, connection): (Option<ChatId>, Option<Arc<str>>),
  names: &ProjectNames,
) -> TurnEnv {
  let open = project.open.clone();
  let stop = view.stop.clone();
  let ask_again = match chat {
    Some(chat) => view.ask_again(project, chat),
    // A new conversation's first message is still being sent: nothing ended there to ask again.
    None => Arc::new(|_: Arc<str>| {}),
  };
  let scope = chat.map(|chat| ChatScope {
    project: open.project.id(),
    name: open.project.name().as_str().to_owned(),
    root: open.repo_root.clone(),
    chat,
    on_its_own: false,
  });
  let (queued, root) = (view.queued.clone(), open.repo_root.clone());
  let picker = view.runs_on.clone();
  TurnEnv {
    project: open.project.id(),
    name: Arc::from(open.project.name().as_str()),
    root: open.repo_root.clone(),
    connection,
    scope,
    stop: Arc::new(move || {
      if let Some(chat) = chat {
        stop.run((open.clone(), chat));
      }
    }),
    ask_again,
    pick_account: Arc::new(move || picker.set(true)),
    change_queued: Arc::new(move |message, text: Arc<str>, change| {
      if !queued.state().get_untracked().is_pending() {
        queued.run(ChangeQueued {
          root: root.clone(),
          message,
          text: text.to_string(),
          change,
        });
      }
    }),
    links: view.links.clone(),
    names: names.clone(),
  }
}
