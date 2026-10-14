//! What the Orchestrator screen reads: what the Orchestrator runs on, History's conversations and the conversation
//! shown, each again whenever the project's runner or the chat's tools change a conversation.

use std::path::{Path, PathBuf};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::id::ChatId;
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    chat::{ChatView, History, history_in},
    settings::OrchestratorSetup,
  },
  runner::Runners,
};

use crate::{project::hold::ReadHold, routes::OrchestratorChat, services::Services};

/// A reading, as far as it went.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Read<T> {
  Loading,
  Failed(Message),
  Ready(T),
}

impl<T> Read<T> {
  pub(super) const fn ready(&self) -> Option<&T> {
    match self {
      Self::Ready(value) => Some(value),
      Self::Loading | Self::Failed(_) => None,
    }
  }
}

/// The conversation the screen shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Shown {
  /// A new conversation, not kept until its first message is sent.
  New,
  Loading,
  Failed(Message),
  /// The route names a conversation the project does not keep.
  Missing,
  Chat(ChatView),
}

/// Everything the screen read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Reads {
  pub(super) setup: Read<OrchestratorSetup>,
  pub(super) history: Read<History>,
  /// The conversation the route names, or the latest when it names none; `None` for a new one.
  pub(super) target: Option<ChatId>,
  pub(super) shown: Shown,
}

/// Reads what the screen of the project at `root` shows for `chat`.
pub(super) fn read(ctx: &mut Ctx, root: &Path, chat: OrchestratorChat) -> Reads {
  let services = Services::of(ctx);
  let revision = revision(ctx, &services.runners, root);
  let hold = services.hold;
  let setup = read_with(ctx, (root.to_path_buf(), revision), &hold, |root| {
    OrchestratorSetup::load_in(&root)
  });
  let history = read_with(ctx, (root.to_path_buf(), revision), &hold, |root| history_in(&root));
  let target = match chat {
    OrchestratorChat::New => None,
    OrchestratorChat::Chat(id) => Some(id),
    OrchestratorChat::Latest => history
      .ready()
      .and_then(|history| history.chats.first())
      .map(|listing| listing.chat.id()),
  };
  let shown = match (chat, target) {
    (OrchestratorChat::Latest, None) if matches!(history, Read::Loading) => Shown::Loading,
    (_, None) => Shown::New,
    (_, Some(id)) => conversation(ctx, (root.to_path_buf(), id, revision), &hold),
  };
  Reads {
    setup,
    history,
    target,
    shown,
  }
}

/// How often the project's conversations changed.
fn revision(ctx: &mut Ctx, runners: &Runners, root: &Path) -> u64 {
  let count = ctx.stream(
    (runners.clone(), root.to_path_buf()),
    |(runners, root), emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = runners.chats_of(&root);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// `load` of the project at `root`, read off the UI thread again whenever `revision` changes; the value last read
/// stays while it is read again.
fn read_with<T>(
  ctx: &mut Ctx,
  (root, revision): (PathBuf, u64),
  hold: &ReadHold,
  load: fn(PathBuf) -> Result<T, Message>,
) -> Read<T>
where
  T: Clone + PartialEq + Send + Sync + 'static + lurq::core::SignalValue,
{
  let hold = hold.clone();
  let state = ctx
    .future((root, revision), move |(root, _)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || load(root)).await.and_then(|read| read)
      }
    })
    .state()
    .get();
  match (state.data, state.error) {
    (Some(value), _) => Read::Ready(value),
    (None, Some(error)) => Read::Failed(error),
    (None, None) => Read::Loading,
  }
}

/// Conversation `id` of the project at `root`.
fn conversation(ctx: &mut Ctx, (root, id, revision): (PathBuf, ChatId, u64), hold: &ReadHold) -> Shown {
  let hold = hold.clone();
  let state = ctx
    .future((root, id, revision), move |(root, id, _)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || ChatView::load_in(&root, id))
          .await
          .and_then(|read| read)
      }
    })
    .state()
    .get();
  let pending = state.is_pending();
  match (state.data, state.error) {
    // A conversation read before stays while it is read again; another one's does not.
    (Some(Some(view)), _) if view.chat.id() == id => Shown::Chat(view),
    (Some(None), _) if !pending => Shown::Missing,
    (None, Some(error)) => Shown::Failed(error),
    _ => Shown::Loading,
  }
}
