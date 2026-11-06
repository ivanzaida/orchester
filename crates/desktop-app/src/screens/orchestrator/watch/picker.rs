//! What the /watch-module picker offers (`R1s1o` in 02 `c8pALl`): the project's open modules whose name holds the words
//! after the chips, then the tasks without a module and the whole project, as `WatchCatalog::modules` lists them for
//! the conversation shown. The catalog — the project's modules and tasks and who leads what — is read off the UI
//! thread, again whenever a conversation or the project's tasks change. A module already chosen, or with nothing left
//! to watch, cannot be chosen; one another conversation leads can, and the command returns it for the card.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::WatchTarget,
};
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  background,
  mentions::watch::{Choosing, Leader, Leaders, WatchCatalog, modules::ModuleEntry},
  messages::{self, Message},
  tasks::data::ProjectTasks,
};

use super::{
  key_of,
  rows::{self, Shown},
  target_of,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::orchestrator::{data, draft::Conversation},
  services::Services,
  ui::{
    icon::Icon,
    slash::{ArgumentPicker, Asked, Choice, Offer},
  },
};

/// The picker of the project at `root`, for the conversation `of` names.
pub(super) struct ModulePicker {
  root: PathBuf,
  project: ProjectId,
  name: Arc<str>,
  /// Counts saves of the project's tasks: the catalog is read again by it.
  saves: Signal<u64>,
  of: Signal<Option<Conversation>>,
  /// Who chooses while the screen shows a new conversation: an id no conversation has, so nothing reads as led here.
  unkept: ChatId,
}

/// How the catalog's reading stands.
enum Catalog {
  Reading,
  Unreadable(Message),
  Read(Arc<WatchCatalog>),
}

/// The catalog as the picker's reading holds it.
#[derive(Clone, PartialEq)]
struct Read(Arc<WatchCatalog>);

impl DevtoolsInspectable for Read {}

impl ModulePicker {
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

  /// The catalog, read off the UI thread whenever a conversation or the project's tasks changed; the one read last
  /// stays while it is read again.
  fn read(&self, ctx: &mut Ctx) -> Catalog {
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
    match (state.data, state.error) {
      (_, Some(error)) if rejected => Catalog::Unreadable(error),
      (Some(Read(catalog)), _) => Catalog::Read(catalog),
      (None, Some(error)) => Catalog::Unreadable(error),
      (None, None) => Catalog::Reading,
    }
  }

  /// The rows of `catalog` for what was `asked`.
  fn offer_read(&self, ctx: &mut Ctx, asked: &Asked<'_>, catalog: &WatchCatalog) -> Offer {
    let navigation = Navigation::of(ctx);
    let chat = match self.of.get() {
      Some(Conversation::Chat(id)) => id,
      Some(Conversation::New) | None => self.unkept,
    };
    let added: Vec<WatchTarget> = asked.taken.iter().filter_map(|choice| target_of(&choice.key)).collect();
    let choosing = Choosing { chat, added: &added };
    let total = modules_in(&catalog.modules("", choosing));
    let entries = catalog.modules(asked.filter, choosing);
    let offered: Vec<(&ModuleEntry, Choice)> = entries
      .iter()
      .filter_map(|entry| Some((entry, self.choice(ctx, entry)?)))
      .collect();
    let rows = offered
      .iter()
      .map(|(entry, choice)| {
        (
          choice.clone(),
          entry.faded.is_none() && entry.leader != Leader::JustAdded,
        )
      })
      .collect();
    let panel = rows::panel(
      ctx,
      &Shown {
        id: asked.id,
        project: self.project,
        filter: asked.filter,
        offered: &offered,
        focused: asked.focused,
        total,
        navigation,
      },
    );
    Offer { rows, panel }
  }

  /// The chip `entry` becomes: a module by its name, the two others in the picker's words.
  fn choice(&self, ctx: &Ctx, entry: &ModuleEntry) -> Option<Choice> {
    let (icon, label) = match entry.target {
      WatchTarget::Module(_) => (Icon::Boxes, entry.name.clone()?),
      WatchTarget::WithoutModule => (Icon::Inbox, ctx.t("orchestrator.watch.modules.withoutModule")),
      WatchTarget::Project => (
        Icon::FolderKanban,
        ctx.t_args("orchestrator.watch.modules.project", [("project", &*self.name)]),
      ),
      WatchTarget::Task(_) => return None,
    };
    Some(Choice {
      key: key_of(entry.target)?,
      icon,
      label,
    })
  }
}

impl ArgumentPicker for ModulePicker {
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
    match self.read(ctx) {
      Catalog::Read(catalog) => self.offer_read(ctx, asked, &catalog),
      Catalog::Reading => Offer {
        rows: Vec::new(),
        panel: rows::reading(ctx, asked.id),
      },
      Catalog::Unreadable(why) => {
        let runners = Services::of(ctx).runners;
        let root = self.root.clone();
        Offer {
          rows: Vec::new(),
          panel: rows::unreadable(
            ctx,
            (asked.id, &self.name, &why),
            Arc::new(move || runners.chat_changed(&root)),
          ),
        }
      }
    }
  }
}

/// How many of `entries` are modules.
fn modules_in(entries: &[ModuleEntry]) -> usize {
  entries
    .iter()
    .filter(|entry| matches!(entry.target, WatchTarget::Module(_)))
    .count()
}

/// The catalog of the project at `root`. Blocks on SQLite.
fn catalog_in(root: &Path) -> Result<Read, Message> {
  let read = || -> Result<_, StoreError> {
    let db = ProjectDb::open(root)?;
    let tasks = ProjectTasks::read(&db, root, None)?;
    Ok(Read(Arc::new(WatchCatalog::of(&tasks, Leaders::read(&db)?))))
  };
  read().map_err(|error| {
    tracing::error!(%error, "cannot read what the module picker offers");
    messages::store_error(&error)
  })
}
