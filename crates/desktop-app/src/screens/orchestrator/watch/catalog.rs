//! What the /watch pickers choose from, as both read it: the project's modules and tasks and who leads what
//! (`WatchCatalog`), read off the UI thread, again whenever a conversation or the project's tasks change, for the
//! conversation shown. While it is read a picker shows its search and head over bone rows; one that cannot be read
//! says so in the picker, with *Try again*.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::id::{ChatId, ProjectId};
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  background,
  mentions::watch::{Leaders, WatchCatalog},
  messages::{self, Message},
  tasks::data::ProjectTasks,
};

use super::panel::{self, Picker};
use crate::{
  project::ShownProject,
  screens::orchestrator::{data, draft::Conversation},
  services::Services,
  ui::slash::{Asked, Offer},
};

/// The catalog of the project at `root`, for the conversation `of` names.
pub(super) struct CatalogReader {
  root: PathBuf,
  pub(super) project: ProjectId,
  pub(super) name: Arc<str>,
  /// Counts saves of the project's tasks: the catalog is read again by it.
  saves: Signal<u64>,
  of: Signal<Option<Conversation>>,
  /// Who chooses while the screen shows a new conversation: an id no conversation has, so nothing reads as led here.
  unkept: ChatId,
}

/// The catalog as the picker's reading holds it.
#[derive(Clone, PartialEq)]
struct Read(Arc<WatchCatalog>);

impl DevtoolsInspectable for Read {}

impl CatalogReader {
  pub(super) fn new(project: &ShownProject, of: Signal<Option<Conversation>>) -> Self {
    Self {
      root: project.open.repo_root.clone(),
      project: project.open.project.id(),
      name: Arc::from(project.open.project.name().as_str()),
      saves: project.feed.saves.clone(),
      of,
      unkept: ChatId::generate(),
    }
  }

  /// The conversation the command is written in, read tracked.
  pub(super) fn chat(&self) -> ChatId {
    match self.of.get() {
      Some(Conversation::Chat(id)) => id,
      Some(Conversation::New) | None => self.unkept,
    }
  }

  /// `offer_read` of the catalog once it is read; until then `picker`'s head over bone rows, and when it cannot be
  /// read, why, with *Try again*. The catalog read last stays while it is read again.
  pub(super) fn offer(
    &self,
    ctx: &mut Ctx,
    (asked, picker): (&Asked<'_>, Picker),
    offer_read: impl FnOnce(&mut Ctx, &WatchCatalog) -> Offer,
  ) -> Offer {
    let services = Services::of(ctx);
    let revision = data::revision(ctx, &services.runners, &self.root);
    let saves = self.saves.get();
    let hold = services.hold;
    let state = ctx
      .future((self.root.clone(), revision, saves), move |(root, ..)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || catalog_in(&root)).await.and_then(|read| read)
        }
      })
      .state()
      .get();
    let rejected = state.is_rejected();
    let why = match (state.data, state.error) {
      (_, Some(error)) if rejected => error,
      (Some(Read(catalog)), _) => return offer_read(ctx, &catalog),
      (None, Some(error)) => error,
      (None, None) => {
        return Offer {
          rows: Vec::new(),
          panel: panel::reading(ctx, (asked.id, picker), asked.search),
        };
      }
    };
    let runners = services.runners;
    let root = self.root.clone();
    Offer {
      rows: Vec::new(),
      panel: panel::unreadable(
        ctx,
        (asked.id, picker, &self.name, &why),
        asked.search,
        Arc::new(move || runners.chat_changed(&root)),
      ),
    }
  }
}

/// The catalog of the project at `root`. Blocks on SQLite.
fn catalog_in(root: &Path) -> Result<Read, Message> {
  let read = || -> Result<_, StoreError> {
    let db = ProjectDb::open(root)?;
    let tasks = ProjectTasks::read(&db, root, None)?;
    Ok(Read(Arc::new(WatchCatalog::of(&tasks, Leaders::read(&db)?))))
  };
  read().map_err(|error| {
    tracing::error!(%error, "cannot read what the /watch pickers offer");
    messages::store_error(&error)
  })
}
