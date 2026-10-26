//! The version of the task the form's edits are made on ([`Base`]). The fields are filled from it once,
//! or from edits kept on this computer with the version they were made on; every save and Create & run
//! is made on it, so a change made meanwhile, e.g. by automation through Orchester's MCP server, is
//! met by the store's revision check and never overwritten. A newer stored version is taken while the
//! fields hold no edits; under edits that changed other fields they are put on it, and where both
//! changed a field the person chooses (`conflict`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  sharing::TaskRevision,
  task::{
    Task,
    content::{Choice, ContentMerge},
  },
  time::Timestamp,
};
use orchester_workspace::{
  background,
  modules::ModuleChoice,
  tasks::{conflict::EditConflict, run::StartOutcome},
};
use parking_lot::Mutex;

use super::{
  actions::QueuedTask,
  conflict::{Choices, Conflict},
  form::{FormProps, KeptAt, TaskForm},
  kept::{Kept, KeptBase, KeptEdits},
  state::View,
  values::Values,
};
use crate::project::ProjectFeed;

/// The version edits are made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
  /// The task as last read or saved; `None` until New task's task exists.
  pub task: Option<Arc<Task>>,
  /// The revision a save is made on: the task's, or the older one kept edits were made on.
  pub revision: Option<TaskRevision>,
  pub values: Values,
}

impl lurq::app::component::DevtoolsInspectable for Base {}

impl Base {
  fn new_task() -> Self {
    Self {
      task: None,
      revision: None,
      values: Values::default(),
    }
  }

  pub fn of(task: &Arc<Task>, separator: &str) -> Self {
    Self {
      task: Some(task.clone()),
      revision: Some(task.revision()),
      values: Values::of_task(task, separator),
    }
  }

  fn kept(&self) -> KeptBase {
    KeptBase {
      revision: self.revision,
      values: self.values.clone(),
    }
  }

  fn revision_number(&self) -> u64 {
    self.revision.map_or(0, TaskRevision::get)
  }

  /// The module the task is stored in; none for a task not made yet.
  pub fn module(&self) -> Option<orchester_core::id::ModuleId> {
    self.task.as_ref().and_then(|task| task.module())
  }
}

/// A newer version of the task, stored while the fields hold edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Newer(pub Arc<Task>);

impl lurq::app::component::DevtoolsInspectable for Newer {}

/// What reading the kept edits found.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeptRead(Option<Kept>);

impl lurq::app::component::DevtoolsInspectable for KeptRead {}

/// What a form last wrote to its kept edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Written {
  /// That none are kept.
  #[default]
  Nothing,
  Values(Values),
  /// The form created its task or queued it: nothing is kept again.
  Finished,
}

impl TaskForm {
  /// Fills the fields once the kept edits are read: from them when they hold edits, made on the
  /// version they were made on, else from what is stored. Whether the fields are filled.
  pub(super) fn seed(&self, ctx: &mut Ctx, props: &FormProps, separator: &str) -> bool {
    let (kept, tasks) = (self.kept.clone(), props.tasks.clone());
    let read = ctx.future((), move |()| {
      let (kept, tasks) = (kept.clone(), tasks.clone());
      async move { background::run(move || KeptRead(kept.read(&tasks))).await }
    });
    if self.seeded.get_untracked() {
      return true;
    }
    let state = read.state().get();
    let kept = match (state.data, state.error) {
      (Some(KeptRead(kept)), _) => kept,
      // Background work that did not finish loses the kept edits; the form opens on what is stored.
      (None, Some(_)) => None,
      (None, None) => return false,
    };
    let stored = props
      .subject
      .task()
      .map_or_else(Base::new_task, |task| Base::of(task, separator));
    if let Some(task) = props.subject.task().filter(|_| props.subject.awaits_first_run()) {
      // A queued task's details are locked: edits kept for it from an earlier visit cannot be saved.
      if kept.is_some() {
        self.kept.remove();
      }
      self.queued.set(Some(QueuedTask {
        task: task.id(),
        key: task.key().clone(),
        outcome: StartOutcome::ConfirmedNotStarted,
        unsourced: Vec::new(),
      }));
      self.start_from(&stored);
    } else {
      match kept.filter(|kept| kept.values.differ_from(&kept.base.values, separator)) {
        Some(kept) => self.restore(kept, stored, (separator, props.updated_at)),
        None => self.start_from(&stored),
      }
    }
    self.seeded.set(true);
    true
  }

  /// Fills the fields with `base` and makes it the version edits are made on; a failure met on an
  /// earlier version is not said any more.
  fn start_from(&self, base: &Base) {
    self.fields.fill(&base.values);
    self.fields.module.set(ModuleChoice(base.module()));
    self.base.set(Some(base.clone()));
    self.dismissed.set(true);
  }

  /// Fills in edits kept earlier, made on the version they were made on: when the task changed since,
  /// a save meets the change, and fields both changed are the person's to choose. The module is the stored
  /// task's: kept edits hold its details only.
  fn restore(&self, kept: Kept, stored: Base, (separator, at): (&str, Option<Timestamp>)) {
    let made_on = kept.base.revision.map_or(0, TaskRevision::get);
    let newer = (made_on < stored.revision_number())
      .then(|| stored.task.clone())
      .flatten();
    let base = match &newer {
      Some(_) => Base {
        task: stored.task,
        revision: kept.base.revision,
        values: kept.base.values,
      },
      None => stored,
    };
    self.fields.fill(&kept.values);
    self.fields.module.set(ModuleChoice(base.module()));
    self.base.set(Some(base.clone()));
    self.kept_at.set(Some(KeptAt(kept.at)));
    // This form keeps them from now on, as it would have written them: it removes them once saved.
    self.kept.write(Some((&kept.values, &base.kept(), kept.at)));
    self.kept.flush();
    *self.written.lock() = Written::Values(kept.values);
    if let Some(newer) = newer {
      self.meet_newer(&newer, &base, (separator, at));
    }
  }

  /// Takes a newer stored version of the task while the fields hold no edits; under edits puts them on
  /// it ([`Self::meet_newer`]). Not while a save or a run request is under way, which settle the
  /// version themselves, nor while the person chooses between versions: saving the choice meets a
  /// change made meanwhile.
  pub(super) fn follow_stored(&self, props: &FormProps, separator: &str, busy: bool) {
    let (Some(task), Some(base)) = (props.subject.task(), self.base.get_untracked()) else {
      return;
    };
    let choosing = self.conflict.get_untracked().is_some();
    if busy || choosing || task.revision().get() <= base.revision_number() {
      return;
    }
    if !self.fields.values_untracked().differ_from(&base.values, separator) {
      self.start_from(&Base::of(task, separator));
      self.newer.set(None);
      self.kept_at.set(None);
    } else if self.newer.get_untracked().as_ref().map(|newer| &newer.0) != Some(task) {
      self.meet_newer(task, &base, (separator, props.updated_at));
    }
  }

  /// A newer version `task` came while the fields hold edits made on `base`: when the two changed
  /// different fields, the edits are put on it without asking; else the person chooses per field. Edits
  /// that are not valid yet are compared once they are saved. *Don't merge automatically*, the priority and
  /// what the runs deliver are the organization's and merge by themselves: the person's choice when they
  /// changed one, else the newer one's.
  fn meet_newer(&self, task: &Arc<Task>, base: &Base, (separator, at): (&str, Option<Timestamp>)) {
    self.newer.set(Some(Newer(task.clone())));
    let values = self.fields.values_untracked();
    let made_on = base.values.task_form(separator).details().ok();
    let mine = values.task_form(separator).details().ok();
    let (Some(made_on), Some(mine)) = (made_on, mine) else {
      return;
    };
    let merge = ContentMerge::new(made_on, mine, task.details().clone());
    if merge.is_clean() {
      let organization = values.organization(&base.values, task.organization());
      // Story points are not part of the details: the person's choice in the field stays.
      self.fields.fill(&Values {
        no_automatic_merge: organization.no_automatic_merge(),
        priority: organization.priority(),
        delivers: organization.delivers(),
        points: values.points,
        ..Values::of_details(&merge.merged(|_| Choice::Mine), separator)
      });
      self.base.set(Some(Base::of(task, separator)));
      self.newer.set(None);
      self.dismissed.set(true);
      return;
    }
    let organization = task.organization().clone();
    self.conflict.set(Some(Conflict(Arc::new(EditConflict {
      merge,
      current: task.clone(),
      by: None,
      at,
      organization: (organization.clone(), organization),
    }))));
    self.choices.set(Choices::default());
  }

  /// Keeps the fields' values on this computer while they differ from the version they were made on,
  /// and removes them once they do not.
  pub(super) fn keep(&self, view: &View, feed: &ProjectFeed) {
    let task = view.subject.task().map(|task| task.key().to_string());
    let Some(base) = self.base.get_untracked().filter(|_| !view.queued()) else {
      feed.mark_unsaved(task.as_deref(), false);
      return;
    };
    let wanted = if view.changed {
      Written::Values(view.values.clone())
    } else {
      Written::Nothing
    };
    let mut written = self.written.lock();
    // The tab of a form whose edits are kept is marked (`Fd1ai`).
    feed.mark_unsaved(task.as_deref(), *written != Written::Finished && view.changed);
    if *written == Written::Finished || *written == wanted {
      return;
    }
    match &wanted {
      Written::Values(values) => self.kept.write(Some((values, &base.kept(), Timestamp::now()))),
      Written::Nothing | Written::Finished => self.kept.write(None),
    }
    *written = wanted;
  }

  /// Removes the kept edits once the form created its task or queued it: they are stored now.
  pub(super) fn forget_kept(kept: &KeptEdits, written: &Mutex<Written>) {
    kept.remove();
    *written.lock() = Written::Finished;
  }
}
