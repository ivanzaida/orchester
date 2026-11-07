//! What the Orchestrator screen reads: what the Orchestrator runs on, History's conversations, the conversation
//! shown and the names of the projects the app knows (for what a link to another project's place says), each again
//! whenever the project's runner or the chat's tools change a conversation; the conversation shown, and what it
//! watches (`/watching`, `super::watching`), also whenever a run changes state or the project's tasks are saved (a
//! merge in Orchester among them), which may end a wait it keeps; and the conversation shown also as the runs'
//! transcripts grow while a stall of a run stands in it, which settles once that run progresses (ORC-894).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use lurq::{
  app::ctx::{Ctx, StreamEmitter},
  core::Signal,
};
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

use crate::{project::hold::ReadHold, routes::OrchestratorChat, services::Services, ui::chat::names::ProjectNames};

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

/// Reads what the screen of the project at `root` shows for `chat`, its tasks saved `saves` times so far; `stalled`
/// holds whether a stall stood in the conversation as it was last read.
pub(super) fn read(
  ctx: &mut Ctx,
  (root, saves): (&Path, u64),
  (chat, stalled): (OrchestratorChat, &Signal<bool>),
) -> Reads {
  let services = Services::of(ctx);
  let revision = revision(ctx, &services.runners, root);
  // A run that changes state, or that Orchester merged (a save of the tasks), may end a wait the conversation keeps:
  // it is read again to say so.
  let states = counted(ctx, (&services.runners, root), Runners::states_of) + saves;
  // A stall the conversation keeps settles once its run progresses, which only its transcript may tell (ORC-894):
  // while one stands, the conversation is read again on the runs' transcripts too, at most once per `STALL_PACE`.
  let progressed = progress(ctx, (services.runners.clone(), root.to_path_buf(), stalled.get()));
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
      .and_then(|history| history.listed().next())
      .map(|listing| listing.chat.id()),
  };
  // Read on every pass, a new conversation's too, so the screen's hooks keep their order.
  let read = conversation(
    ctx,
    (root.to_path_buf(), target, (revision, states + progressed)),
    &hold,
  );
  let stands = matches!(&read, Shown::Chat(view) if view.stall_stands());
  if stalled.get_untracked() != stands {
    stalled.set(stands);
  }
  let watching = watching(ctx, (root.to_path_buf(), target, (revision, states)), &hold);
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

/// The count of project `root`'s changes that `of` follows, whenever it changes.
fn counted(ctx: &mut Ctx, (runners, root): (&Runners, &Path), of: fn(&Runners, &Path) -> watch::Receiver<u64>) -> u64 {
  let count = ctx.stream(
    (runners.clone(), root.to_path_buf()),
    move |(runners, root), emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = of(&runners, &root);
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

/// How long the count of the runs' transcript changes stays put after it moved, while a stall stands: reading the
/// conversation again takes more than a transcript's view does, and a stall that settles a little later is still told.
const STALL_PACE: Duration = Duration::from_secs(2);

/// How often project `root`'s runs or their transcripts changed, followed only while `follow` (a stall stands); `0`
/// before.
fn progress(ctx: &mut Ctx, deps: (Runners, PathBuf, bool)) -> u64 {
  let count = ctx.stream(
    deps,
    |(runners, root, follow), emitter: StreamEmitter<u64, ()>| async move {
      if !follow {
        return;
      }
      let mut updates = runners.changes_of(&root);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) {
          return;
        }
        tokio::time::sleep(STALL_PACE).await;
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// `load` of the project at `root`, read off the UI thread again whenever `revision` changes; the value last read
/// stays while it is read again, and a reading that failed says so even when an earlier one succeeded.
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
  let rejected = state.is_rejected();
  match (state.data, state.error) {
    (_, Some(error)) if rejected => Read::Failed(error),
    (Some(value), _) => Read::Ready(value),
    (None, Some(error)) => Read::Failed(error),
    (None, None) => Read::Loading,
  }
}

/// Conversation `id` of the project at `root`, read again whenever `revision` (the conversations' and the runs'
/// states') changes; nothing is read for a new one (`None`).
fn conversation(ctx: &mut Ctx, (root, id, revision): (PathBuf, Option<ChatId>, (u64, u64)), hold: &ReadHold) -> Shown {
  let hold = hold.clone();
  let state = ctx
    .future((root, id, revision), move |(root, id, _)| {
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
    })
    .state()
    .get();
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
/// stored state again whenever `revision` (the conversations' and the runs' states, the tasks' saves) changes; `None`
/// for a new one. What another conversation was read to lead does not stand for this one's.
fn watching(
  ctx: &mut Ctx,
  (root, id, revision): (PathBuf, Option<ChatId>, (u64, u64)),
  hold: &ReadHold,
) -> Option<Read<Arc<Watching>>> {
  let hold = hold.clone();
  let state = ctx
    .future((root, id, revision), move |(root, id, _)| {
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
    })
    .state()
    .get();
  let id = id?;
  let rejected = state.is_rejected();
  Some(match (state.data, state.error) {
    (_, Some(error)) if rejected => Read::Failed(error),
    (Some(Some((read, watching))), _) if read == id => Read::Ready(watching),
    (None, Some(error)) => Read::Failed(error),
    _ => Read::Loading,
  })
}
