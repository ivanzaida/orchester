//! Edit task while the task is changed elsewhere, e.g. by automation through Orchester's MCP server:
//! the project's tasks are read again and the form is handed the newer version. A form without edits
//! takes its values; a form with edits keeps them and says quietly that the task changed, with
//! *Reload*, which shows the stored task in place of the edits. Either way the next save is made on
//! the version the fields were filled from ([`EditBase`]), so the store sees the change made meanwhile
//! and merges it, or the form shows the conflict: a save never reverts it silently.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  sharing::TaskRevision,
  task::{Task, TaskDetails},
};

use super::{Editing, FormState, NewTaskForm, Reseed};
use crate::{
  theme::ExtraSpacing,
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// The version of the task the fields were filled from, or that the form's last save stored: what a
/// save is made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EditBase {
  pub(super) revision: TaskRevision,
  pub(super) details: TaskDetails,
}

impl lurq::app::component::DevtoolsInspectable for EditBase {}

impl EditBase {
  pub(super) fn of(task: &Task) -> Self {
    Self {
      revision: task.revision(),
      details: task.details().clone(),
    }
  }
}

/// What the form knows of the version it edits: the one its fields were filled from, and whether a
/// newer one came while they hold edits.
#[derive(Clone)]
pub(super) struct Followed {
  pub(super) base: Signal<Option<EditBase>>,
  changed: Signal<bool>,
  /// The form's own: what the fields show next.
  reseed: Signal<Option<Reseed>>,
}

impl Followed {
  pub(super) fn new(ctx: &mut Ctx, reseed: Signal<Option<Reseed>>) -> Self {
    Self {
      base: ctx.signal(None),
      changed: ctx.signal(false),
      reseed,
    }
  }

  /// The form saved `details`, stored as `revision`: the next save is made on them, and the newer
  /// version the notice spoke of is part of them.
  pub(super) fn saved(&self, revision: TaskRevision, details: TaskDetails) {
    self.base.set(Some(EditBase { revision, details }));
    if self.changed.get_untracked() {
      self.changed.set(false);
    }
  }

  /// Fills the fields with `task` as stored, which the next save is made on.
  fn take_stored(&self, task: &Task) {
    self.base.set(Some(EditBase::of(task)));
    self.reseed.set(Some(Reseed {
      details: task.details().clone(),
      organization: Some(task.organization().clone()),
    }));
    self.changed.set(false);
  }
}

impl NewTaskForm {
  /// Takes a newer version of the task being edited when the fields hold no edits; otherwise keeps
  /// them and shows the notice. Not while the form saves, starts a run or shows a conflict: those
  /// settle the version themselves.
  pub(super) fn follow_outside(&self, editing: Option<&Editing>, state: &FormState) {
    let followed = &self.followed;
    let (Some(editing), Some(base)) = (editing, followed.base.get_untracked()) else {
      return;
    };
    let task = &editing.task;
    let busy = state.saving || state.starting || state.queued || self.conflict.get_untracked().is_some();
    if task.revision().get() <= base.revision.get() || busy {
      return;
    }
    let holds_base = state.ready.as_ref() == Some(&base.details)
      && self.organization.change().is_some_and(|(loaded, now)| loaded == now);
    if holds_base {
      followed.take_stored(task);
    } else if !followed.changed.get_untracked() {
      followed.changed.set(true);
    }
  }

  /// The notice that the task changed while the person edits it, with *Reload*.
  pub(super) fn outside_notice(&self, ctx: &Ctx, editing: Option<&Editing>) -> Option<Element> {
    let editing = editing.filter(|_| self.followed.changed.get())?;
    let (followed, task) = (self.followed.clone(), editing.task.clone());
    let reload = ActionButton::new(ctx.t("common.reload"))
      .id("edit-task-reload")
      .on_click(move || followed.take_stored(&task))
      .render(ctx);
    let key = editing.task.key().to_string();
    let notice = Notice::new(
      Icon::RefreshCw,
      ctx.t("tasks.edit.changedOutside"),
      ctx.t_args("tasks.edit.changedOutsideDetail", [("task", key.as_str())]),
    )
    .id("edit-task-changed-outside")
    .action(reload);
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_horizontal(ExtraSpacing::Indent.role())
        .padding_bottom(SpacingSize::Md)
        .child(notice)
        .into(),
    )
  }
}
