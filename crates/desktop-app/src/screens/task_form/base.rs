//! The version of the task the form's edits are made on ([`Base`]). The fields are filled from it once,
//! or from edits kept on this computer with the version they were made on; every save and Create & run
//! is made on it, so a change made meanwhile, e.g. by automation through Orchester's MCP server, is
//! met by the store's revision check and never overwritten. A newer stored version is taken while the
//! fields hold no edits; under edits the form keeps them and says the task changed, with *Reload*
//! (as the deprecated desktop's `new_task/outside.rs` does).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{sharing::TaskRevision, task::Task, time::Timestamp};
use orchester_workspace::{background, tasks::run::StartOutcome};
use parking_lot::Mutex;

use super::{
  actions::QueuedTask,
  form::{FormProps, KeptAt, TaskForm},
  kept::{Kept, KeptBase, KeptEdits},
  state::View,
  values::Values,
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  notice::{Notice, NoticeTone},
};

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
      values: Values::of_details(task.details(), separator),
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
      self.queued.set(Some(QueuedTask {
        task: task.id(),
        key: task.key().clone(),
        outcome: StartOutcome::ConfirmedNotStarted,
      }));
      self.start_from(&stored);
    } else {
      match kept.filter(|kept| kept.values.differ_from(&kept.base.values, separator)) {
        Some(kept) => self.restore(kept, stored),
        None => self.start_from(&stored),
      }
    }
    self.seeded.set(true);
    true
  }

  fn start_from(&self, base: &Base) {
    self.fields.fill(&base.values);
    self.base.set(Some(base.clone()));
  }

  /// Fills in edits kept earlier, made on the version they were made on: when the task changed since,
  /// a save meets the change, and the form says so with Reload.
  fn restore(&self, kept: Kept, stored: Base) {
    let made_on = kept.base.revision.map_or(0, TaskRevision::get);
    let base = if made_on < stored.revision_number() {
      self.newer.set(stored.task.clone().map(Newer));
      Base {
        task: stored.task,
        revision: kept.base.revision,
        values: kept.base.values,
      }
    } else {
      stored
    };
    self.fields.fill(&kept.values);
    self.base.set(Some(base));
    self.kept_at.set(Some(KeptAt(kept.at)));
    *self.written.lock() = Written::Values(kept.values);
  }

  /// Takes a newer stored version of the task while the fields hold no edits; under edits keeps them
  /// and remembers the newer version for the notice. Not while a save or a run request is under way:
  /// those settle the version themselves.
  pub(super) fn follow_stored(&self, props: &FormProps, separator: &str, busy: bool) {
    let (Some(task), Some(base)) = (props.subject.task(), self.base.get_untracked()) else {
      return;
    };
    if busy || task.revision().get() <= base.revision_number() {
      return;
    }
    if !self.fields.values_untracked().differ_from(&base.values, separator) {
      self.start_from(&Base::of(task, separator));
      self.newer.set(None);
      self.kept_at.set(None);
    } else if self.newer.get_untracked().as_ref().map(|newer| &newer.0) != Some(task) {
      self.newer.set(Some(Newer(task.clone())));
    }
  }

  /// Keeps the fields' values on this computer while they differ from the version they were made on,
  /// and removes them once they do not.
  pub(super) fn keep(&self, view: &View) {
    let Some(base) = self.base.get_untracked().filter(|_| !view.queued()) else {
      return;
    };
    let wanted = if view.changed {
      Written::Values(view.values.clone())
    } else {
      Written::Nothing
    };
    let mut written = self.written.lock();
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
    kept.write(None);
    *written.lock() = Written::Finished;
  }

  /// The notice that the task changed while the fields hold edits, with Reload (`outside.rs` of the
  /// deprecated desktop).
  pub(super) fn newer_notice(&self, ctx: &Ctx, view: &View, separator: &str) -> Option<Element> {
    let newer = self.newer.get().filter(|_| !view.locked())?;
    let (fields, base, latest) = (self.fields.clone(), self.base.clone(), self.newer.clone());
    let (kept_at, separator) = (self.kept_at.clone(), separator.to_owned());
    let task = newer.0.clone();
    let reload = ActionButton::new(
      "task-form-reload",
      ButtonKind::Secondary,
      ctx.t("task_form.newer.reload"),
    )
    .in_form()
    .on_click(move || {
      let stored = Base::of(&task, &separator);
      fields.fill(&stored.values);
      base.set(Some(stored));
      latest.set(None);
      kept_at.set(None);
    });
    let key = newer.0.key().to_string();
    Some(
      Notice::new("task-form-newer", NoticeTone::Warning, ctx.t("task_form.newer.title"))
        .icon(Icon::History)
        .body(ctx.t_args("task_form.newer.body", [("key", key)]))
        .action(reload)
        .into(),
    )
  }
}
