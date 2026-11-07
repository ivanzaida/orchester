//! What the /watch-module picker offers (`R1s1o` in 02 `c8pALl`): the project's open modules whose name holds the words
//! after the chips, then the tasks without a module and the whole project, as `WatchCatalog::modules` lists them for
//! the conversation shown ([`super::catalog`]). A module already chosen, or with nothing left to watch, cannot be
//! chosen; one another conversation leads can, and the command returns it for the card.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::orchestrator::WatchTarget;
use orchester_workspace::mentions::watch::{Choosing, Leader, WatchCatalog, modules::ModuleEntry};

use super::{
  catalog::CatalogReader,
  module::{key_of, target_of},
  panel::Picker,
  rows::{self, Shown},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::orchestrator::draft::Conversation,
  ui::{
    icon::Icon,
    slash::{ArgumentPicker, Asked, Choice, Offer},
  },
};

/// The picker of the shown project's modules, for the conversation `of` names.
pub(super) struct ModulePicker {
  catalog: CatalogReader,
}

impl ModulePicker {
  pub(super) fn new(project: &ShownProject, of: Signal<Option<Conversation>>) -> Self {
    Self {
      catalog: CatalogReader::new(project, of),
    }
  }

  /// The rows of `catalog` for what was `asked`.
  fn offer_read(&self, ctx: &mut Ctx, asked: &Asked<'_>, catalog: &WatchCatalog) -> Offer {
    let navigation = Navigation::of(ctx);
    let added: Vec<WatchTarget> = asked.taken.iter().filter_map(|choice| target_of(&choice.key)).collect();
    let choosing = Choosing {
      chat: self.catalog.chat(),
      added: &added,
    };
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
        project: self.catalog.project,
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
        ctx.t_args("orchestrator.watch.modules.project", [("project", &*self.catalog.name)]),
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
    self.catalog.offer(ctx, (asked, Picker::Module), |ctx, catalog| {
      self.offer_read(ctx, asked, catalog)
    })
  }
}

/// How many of `entries` are modules.
fn modules_in(entries: &[ModuleEntry]) -> usize {
  entries
    .iter()
    .filter(|entry| matches!(entry.target, WatchTarget::Module(_)))
    .count()
}
