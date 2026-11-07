//! `/watch-task` (ORC-773, `03 · Choosing tasks` `mQdxa` in `Orchestrator watch · redesign`; its picker v2 as `P1`
//! `QXPDY`, ORC-889, its badge P4 `nFeQs`, ORC-890): chosen in the slash menu it becomes the composer's *Watch* badge
//! and opens the picker of the project's tasks, matched by key or title in its own search as `WatchCatalog::tasks`
//! lists them for the conversation shown ([`super::catalog`], [`super::task_rows`]). The tasks chosen there join the
//! badge by their keys, and the composer's send has the conversation watch them with the message's words as their
//! order, as `/watch-module` does ([`super::module::apply`]). A Done or Canceled task, or one chosen already, cannot be
//! chosen; while the search is empty the Done and Canceled ones hide behind one row that counts them. One another
//! conversation leads can be chosen, and the command returns it for the card.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::TaskId,
  orchestrator::{WatchTarget, watch::NamedTarget},
};
use orchester_workspace::mentions::watch::{Choosing, Leader, WatchCatalog, tasks::TaskEntry};

use super::{
  catalog::CatalogReader,
  module::apply,
  panel::Picker,
  rows::Counts,
  task_rows::{self, Shown},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::orchestrator::View,
  ui::{
    icon::Icon,
    slash::{ArgumentPicker, Asked, Badge, Choice, Does, Offer, SlashCommand, Taking},
  },
};

/// The command's name, which its picker's elements are named after.
pub(in crate::screens::orchestrator) const TASK: &str = "watch-task";

/// *Watch tasks* in the slash menu: it takes the tasks chosen in its picker, and its badge says *Watch* with the
/// module's `eye` (P4).
pub(in crate::screens::orchestrator) fn task_command(ctx: &Ctx, view: &View, project: &ShownProject) -> SlashCommand {
  let picker = TaskPicker {
    catalog: CatalogReader::new(project, view.of.clone()),
  };
  SlashCommand {
    name: TASK,
    icon: Icon::ScanEye,
    label: ctx.t("composer.slash.command.watchTask.label"),
    detail: ctx.t("composer.slash.command.watchTask.detail"),
    does: Some(Does::Take(Taking {
      picker: Arc::new(picker),
      apply: apply(view, project.open.repo_root.clone(), named_task),
      badge: Badge {
        icon: Icon::Eye,
        verb: ctx.t("composer.slash.command.watchTask.badge"),
      },
    })),
  }
}

/// The task a chip of the picker names, by its key.
fn named_task(choice: &Choice) -> Option<NamedTarget> {
  Some(NamedTarget {
    target: target_of(&choice.key)?,
    name: Some(choice.label.to_string()),
  })
}

/// The task a chip's key names: its id.
fn target_of(key: &str) -> Option<WatchTarget> {
  key.parse::<TaskId>().ok().map(WatchTarget::Task)
}

/// The picker of the shown project's tasks, for the conversation `of` names.
struct TaskPicker {
  catalog: CatalogReader,
}

impl TaskPicker {
  /// The rows of `catalog` for what was `asked`; while the search is empty, without the Done and Canceled tasks.
  fn offer_read(&self, ctx: &mut Ctx, asked: &Asked<'_>, catalog: &WatchCatalog) -> Offer {
    let navigation = Navigation::of(ctx);
    let added: Vec<WatchTarget> = asked.taken.iter().filter_map(|choice| target_of(&choice.key)).collect();
    let choosing = Choosing {
      chat: self.catalog.chat(),
      added: &added,
    };
    let entries = catalog.tasks(asked.filter, choosing);
    let hide = asked.filter.trim().is_empty();
    let offered: Vec<(&TaskEntry, Choice)> = entries
      .iter()
      .filter(|entry| !(hide && entry.faded.is_some()))
      .map(|entry| (entry, choice(entry)))
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
    let all = catalog.tasks("", choosing);
    let counts = Counts {
      open: all.iter().filter(|entry| entry.faded.is_none()).count(),
      added: asked.taken.len(),
      total: catalog.tasks.len(),
      hidden: if hide { entries.len() - offered.len() } else { 0 },
    };
    let shown = Shown {
      id: asked.id,
      project: self.catalog.project,
      filter: asked.filter,
      offered: &offered,
      focused: asked.focused,
      counts,
      navigation,
    };
    let panel = task_rows::panel(ctx, &shown, asked.search);
    Offer { rows, panel }
  }
}

impl ArgumentPicker for TaskPicker {
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer {
    self.catalog.offer(ctx, (asked, Picker::Task), |ctx, catalog| {
      self.offer_read(ctx, asked, catalog)
    })
  }
}

/// The chip `entry` becomes: the task by its key.
fn choice(entry: &TaskEntry) -> Choice {
  Choice {
    key: entry.id.to_string().into(),
    icon: Icon::SquareCheck,
    label: entry.key.to_string().into(),
  }
}
