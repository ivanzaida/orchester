//! What the turns' rows need from the screen ([`super::timeline::TurnEnv`]): the conversation's scope and what the
//! person can do about its newest turn and the messages that wait for it, and about its earlier messages.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::id::ChatId;
use orchester_workspace::orchestrator::tools::ChatScope;

use super::{
  View, draft::Conversation, rewind::toolbar::RewindEnv, send::ChangeQueued, timeline::TurnEnv, watch::WatchFacts,
};
use crate::{project::ShownProject, ui::chat::names::ProjectNames};

/// The rows' environment for conversation `chat` of `project`, on `connection` (its name) when the Orchestrator has
/// one, `answering` while a turn of it thinks; `watches` when a line of it names what the conversation watches, which
/// reads the project's tasks.
pub(super) fn env(
  ctx: &Ctx,
  view: &View,
  project: &ShownProject,
  (chat, connection, answering, watches): (Option<ChatId>, Option<Arc<str>>, bool, bool),
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
  // An edit goes as the composer would send a message to the conversation now.
  let on = view.plan_only.on_now(Some(Conversation::from(chat)));
  let sending = view.send.state().get().is_pending();
  let rewind = RewindEnv::new(
    ctx,
    (&view.rewind, &project.open),
    (view.plan_only.mode(on), on),
    sending,
  );
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
    plans: view.plans.clone(),
    open: project.open.clone(),
    change_plan: view.change_plan(),
    changing: view.plan_only.changing.get().map(|changing| changing.seq),
    answering,
    rewind,
    watch: Arc::new(if watches {
      project.feed.tasks.with(|tasks| WatchFacts::of(tasks.ready()))
    } else {
      WatchFacts::default()
    }),
  }
}
