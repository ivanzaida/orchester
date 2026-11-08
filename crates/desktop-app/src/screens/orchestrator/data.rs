//! What the Orchestrator screen reads: what the Orchestrator runs on, History's conversations, the conversation
//! shown and the names of the projects the app knows (for what a link to another project's place says), each again
//! whenever the project's runner or the chat's tools change a conversation; the conversation shown, and what it
//! watches (`/watching`, `super::watching`), also whenever a run changes state or the project's tasks are saved (a
//! merge in Orchester among them), which may end a wait it keeps.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::id::ChatId;
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::{
    chat::{ChatView, History, history_in},
    settings::OrchestratorSetup,
    watching::{WatchReads, Watching},
  },
  runner::Runners,
};
use tokio::sync::watch;

use crate::{
  project::{
    feed::TRANSCRIPT_PACE,
    hold::ReadHold,
    settled_read::{settled_read, settled_read_traced},
  },
  routes::OrchestratorChat,
  services::Services,
  ui::chat::names::ProjectNames,
};

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
  Chat(Box<ChatView>),
}

/// Everything the screen read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Reads {
  pub(super) setup: Read<OrchestratorSetup>,
  pub(super) history: Read<History>,
  /// The conversation the route names, or the latest when it names none; `None` for a new one.
  pub(super) target: Option<ChatId>,
  pub(super) shown: Shown,
  pub(super) names: ProjectNames,
  /// What the target conversation leads and what waits there now; `None` for a new one.
  pub(super) watching: Option<Read<Arc<Watching>>>,
}

/// Reads what the screen of the project at `root` shows for `chat`, its tasks saved `saves` times so far.
pub(super) fn read(ctx: &mut Ctx, (root, saves): (&Path, u64), chat: OrchestratorChat) -> Reads {
  let services = Services::of(ctx);
  let revision = revision(ctx, &services.runners, root);
  // A run that changes state, or that Orchester merged (a save of the tasks), may end a wait the conversation keeps:
  // it is read again to say so.
  let states = counted(ctx, (&services.runners, root), Runners::states_of) + saves;
  let hold = services.hold;
  let setup = read_with(
    ctx,
    ("orchestrator setup", root.to_path_buf(), revision),
    &hold,
    |root| OrchestratorSetup::load_in(&root),
  );
  let history = read_with(
    ctx,
    ("orchestrator history", root.to_path_buf(), revision),
    &hold,
    |root| history_in(&root),
  );
  let target = match chat {
    OrchestratorChat::New => None,
    OrchestratorChat::Chat(id) => Some(id),
    OrchestratorChat::Latest => history
      .ready()
      .and_then(|history| history.listed().next())
      .map(|listing| listing.chat.id()),
  };
  // The conversation shown follows its own changes, not those of the others the Orchestrator answers in.
  let own = own_changes(ctx, (&services.runners, root), target);
  // Read on every pass, a new conversation's too, so the screen's hooks keep their order.
  let read = conversation(ctx, (root.to_path_buf(), target, (own, states)), &hold);
  let watching = watching(ctx, (root.to_path_buf(), target, (own, states)), &hold);
  let names = ProjectNames::read(ctx, services.workspace, revision);
  let shown = match (chat, target) {
    (OrchestratorChat::Latest, None) if matches!(history, Read::Loading) => Shown::Loading,
    (_, None) => Shown::New,
    (_, Some(_)) => read,
  };
  Reads {
    setup,
    history,
    target,
    shown,
    names,
    watching,
  }
}

/// How often the project's conversations changed; a task's conversation reads the chat's proposals again by it too.
pub(in crate::screens) fn revision(ctx: &mut Ctx, runners: &Runners, root: &Path) -> u64 {
  counted(ctx, (runners, root), Runners::chats_of)
}

/// The count of project `root`'s changes that `of` follows, whenever it changes, at most once per
/// [`TRANSCRIPT_PACE`]: an agent answering writes many entries a second, and a screen reads all of them once per pace.
fn counted(ctx: &mut Ctx, (runners, root): (&Runners, &Path), of: fn(&Runners, &Path) -> watch::Receiver<u64>) -> u64 {
  let count = ctx.stream(
    (runners.clone(), root.to_path_buf()),
    move |(runners, root), emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = of(&runners, &root);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) {
          return;
        }
        tokio::time::sleep(TRANSCRIPT_PACE).await;
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// How often conversation `chat` of project `root` changed, at most once per [`TRANSCRIPT_PACE`]; `0` for a new
/// conversation (`None`). Another conversation's changes do not count: the screen reads what it shows when that
/// changed, not each time the Orchestrator writes an entry elsewhere. Asked for in every render, whatever `chat` is:
/// lurq keeps a component's streams by the order it asks for them.
pub(in crate::screens) fn own_changes(ctx: &mut Ctx, (runners, root): (&Runners, &Path), chat: Option<ChatId>) -> u64 {
  let count = ctx.stream(
    (runners.clone(), root.to_path_buf(), chat),
    |(runners, root, chat), emitter: StreamEmitter<u64, ()>| async move {
      let Some(chat) = chat else {
        return;
      };
      let mut updates = runners.chat_changes_of(&root, chat);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) {
          return;
        }
        tokio::time::sleep(TRANSCRIPT_PACE).await;
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// `load` of the project at `root`, read off the UI thread again whenever `revision` changes, never beside a reading
/// that has not ended (`settled_read`); the value last read stays while it is read again, and a reading that failed
/// says so even when an earlier one succeeded. `name` names the read in the read log.
fn read_with<T>(
  ctx: &mut Ctx,
  (name, root, revision): (&'static str, PathBuf, u64),
  hold: &ReadHold,
  load: fn(PathBuf) -> Result<T, Message>,
) -> Read<T>
where
  T: Clone + PartialEq + Send + Sync + 'static + lurq::core::SignalValue,
{
  let hold = hold.clone();
  let state = settled_read(ctx, name, (root, revision), move |(root, _)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || load(root)).await.and_then(|read| read)
    }
  });
  let rejected = state.is_rejected();
  match (state.data, state.error) {
    (_, Some(error)) if rejected => Read::Failed(error),
    (Some(value), _) => Read::Ready(value),
    (None, Some(error)) => Read::Failed(error),
    (None, None) => Read::Loading,
  }
}

/// What a read of one conversation reads by: the project's root, the conversation (none for a new one, which reads
/// nothing) and its change counts.
type ChatDeps = (PathBuf, Option<ChatId>, (u64, u64));

/// Whether the conversation of `deps` is a kept one, which a read has something to read for.
fn has_chat(deps: &ChatDeps) -> bool {
  deps.1.is_some()
}

/// Conversation `id` of the project at `root`, read again whenever `revision` (the conversations' and the runs'
/// states') changes; nothing is read for a new one (`None`).
fn conversation(ctx: &mut Ctx, (root, id, revision): ChatDeps, hold: &ReadHold) -> Shown {
  let hold = hold.clone();
  let state = settled_read_traced(
    ctx,
    ("orchestrator conversation", has_chat),
    (root, id, revision),
    move |(root, id, _)| {
      let hold = hold.clone();
      async move {
        let Some(id) = id else {
          return Ok(None);
        };
        hold.released().await;
        background::run(move || ChatView::load_in(&root, id))
          .await
          .and_then(|read| read)
      }
    },
  );
  let (pending, rejected) = (state.is_pending(), state.is_rejected());
  match (state.data, state.error) {
    (_, Some(error)) if rejected => Shown::Failed(error),
    // A conversation read before stays while it is read again; another one's does not.
    (Some(Some(view)), _) if Some(view.chat.id()) == id => Shown::Chat(Box::new(view)),
    (Some(None), _) if !pending && id.is_some() => Shown::Missing,
    (None, Some(error)) => Shown::Failed(error),
    _ => Shown::Loading,
  }
}

/// What conversation `id` of the project at `root` leads and what waits there (`/watching`, 06 `Ff7AD`), read from the
/// stored state again whenever `revision` (the conversation's own changes, the runs' states, the tasks' saves)
/// changes; `None` for a new one. What another conversation was read to lead does not stand for this one's.
fn watching(ctx: &mut Ctx, (root, id, revision): ChatDeps, hold: &ReadHold) -> Option<Read<Arc<Watching>>> {
  let hold = hold.clone();
  let state = settled_read_traced(
    ctx,
    ("orchestrator watching", has_chat),
    (root, id, revision),
    move |(root, id, _)| {
      let hold = hold.clone();
      async move {
        let Some(id) = id else {
          return Ok(None);
        };
        hold.released().await;
        background::run(move || {
          let reads = ProjectDb::open(&root).and_then(|db| WatchReads::read(&db, &root));
          reads
            .map(|reads| Some((id, Arc::new(reads.watching(id)))))
            .map_err(|error| {
              tracing::error!(%error, "cannot read what the conversation watches");
              messages::store_error(&error)
            })
        })
        .await
        .and_then(|read| read)
      }
    },
  );
  let id = id?;
  let rejected = state.is_rejected();
  Some(match (state.data, state.error) {
    (_, Some(error)) if rejected => Read::Failed(error),
    (Some(Some((read, watching))), _) if read == id => Read::Ready(watching),
    (None, Some(error)) => Read::Failed(error),
    _ => Read::Loading,
  })
}
