//! What the form's actions start, and what the form does once they end: a created task opens Tasks with
//! its row highlighted, a saved edit becomes the version further edits are made on, a save that met a
//! newer version has the project's tasks read again so the form can offer it, a started run opens the
//! task's Conversation, and a task queued without a run keeps the form, locked, with Retry run.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::task::organization::TaskOrganization;
use orchester_workspace::tasks::data::SaveTarget;

use super::{
  actions::{RunAction, RunArgs, SaveAction, SaveArgs, Saved, Started},
  bar::{Handler, Handlers},
  base::{Base, Written},
  form::{FormProps, TaskForm},
  state::{Phase, View},
  values::Values,
};

impl TaskForm {
  pub(super) fn handlers(&self, props: &FormProps, view: &View, (save, run): (&SaveAction, &RunAction)) -> Handlers {
    let base = self.base.get_untracked();
    let made_on = base
      .as_ref()
      .and_then(|base| Some((base.task.clone()?, base.revision?)));
    let args = view.ready.clone().map(|details| SaveArgs {
      repo_root: props.project.open.repo_root.clone(),
      // On the version the edits were made on: a change made meanwhile is met, never overwritten.
      target: made_on
        .as_ref()
        .map_or(SaveTarget::New(self.batch), |(task, revision)| {
          SaveTarget::Task(task.id(), *revision)
        }),
      details,
      organization: made_on
        .as_ref()
        .map_or_else(TaskOrganization::default, |(task, _)| task.organization().clone()),
    });
    let save_handler = args.clone().map(|args| {
      let save = save.clone();
      Arc::new(move || save.run(args.clone())) as Handler
    });
    let (ask, keep) = (self.confirming_discard.clone(), self.confirming_discard.clone());
    Handlers {
      save: save_handler,
      run: self.run_handler(view, run, args),
      ask_discard: Arc::new(move || ask.set(true)),
      keep_editing: Arc::new(move || keep.set(false)),
      discard: self.discard_handler(),
    }
  }

  /// Create & run with the chosen pipeline's published version, or Retry run of the queued task; `None`
  /// while neither can be asked for.
  fn run_handler(&self, view: &View, run: &RunAction, args: Option<SaveArgs>) -> Option<Handler> {
    let (request, retry) = match &view.phase {
      Phase::NotStarted { queued, .. } => (RunArgs::Retry(queued.clone()), true),
      Phase::Editing => {
        let version = view.pipeline()?.version;
        (
          RunArgs::New {
            save: Box::new(args?),
            version,
          },
          false,
        )
      }
      Phase::Saving | Phase::Creating | Phase::Starting(_) => return None,
    };
    let (retrying, run) = (self.retrying.clone(), run.clone());
    Some(Arc::new(move || {
      retrying.set(retry);
      run.run(request.clone());
    }))
  }

  /// Discard changes: the fields show the version they were made on again (New task: empty), and
  /// nothing stays kept.
  fn discard_handler(&self) -> Handler {
    let (fields, base) = (self.fields.clone(), self.base.clone());
    let (kept_at, confirming) = (self.kept_at.clone(), self.confirming_discard.clone());
    Arc::new(move || {
      let values = base.get_untracked().map_or_else(Values::default, |base| base.values);
      fields.fill(&values);
      kept_at.set(None);
      confirming.set(false);
    })
  }

  /// Once a save stored the task: a new one opens Tasks with its row highlighted; an edit is the version
  /// further edits are made on. A save that met a newer version has the tasks read again, so the form
  /// learns of it and offers Reload.
  pub(super) fn follow_save(&self, ctx: &mut Ctx, save: &SaveAction, props: &FormProps) {
    let separator = ctx.t("task_form.number.groupSeparator");
    let (feed, nav, new) = (props.project.feed.clone(), self.nav.clone(), props.subject.is_new());
    let (base, newer, kept_at) = (self.base.clone(), self.newer.clone(), self.kept_at.clone());
    let (kept, written) = (self.kept.clone(), self.written.clone());
    ctx.watch(&save.state(), move |state| {
      if state.is_rejected() {
        feed.saved();
        return;
      }
      let Some(Saved(task)) = state.data.clone().filter(|_| state.is_fulfilled()) else {
        return;
      };
      feed.saved();
      if new && base.get_untracked().is_none_or(|base| base.task.is_none()) {
        Self::forget_kept(&kept, &written);
        nav.tasks(Some(task.key().clone()));
        return;
      }
      newer.set(None);
      kept_at.set(None);
      base.set(Some(Base::of(&task, &separator)));
    });
  }

  /// Once Create & run or Retry run ended: a started run opens the task's Conversation; a task queued
  /// without a run keeps the form, which says why and offers Retry run; a draft saved without being
  /// queued is the one the form edits from now on, never a second task.
  pub(super) fn follow_run(&self, ctx: &mut Ctx, run: &RunAction, props: &FormProps) {
    let separator = ctx.t("task_form.number.groupSeparator");
    let (feed, nav, queued) = (props.project.feed.clone(), self.nav.clone(), self.queued.clone());
    let (kept, written, base) = (self.kept.clone(), self.written.clone(), self.base.clone());
    ctx.watch(&run.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(problems) = state.error.as_ref().filter(|_| state.is_rejected()) {
        feed.saved();
        if let Some(task) = &problems.queued {
          Self::forget_kept(&kept, &written);
          queued.set(Some(task.clone()));
        } else if let Some(Saved(created)) = problems
          .saved
          .clone()
          .filter(|_| base.get_untracked().is_some_and(|base| base.task.is_none()))
        {
          kept.move_to(created.id());
          *written.lock() = Written::Nothing;
          base.set(Some(Base::of(&created, &separator)));
        }
        return;
      }
      if let Some(Started(key)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        Self::forget_kept(&kept, &written);
        nav.started(&key);
      }
    });
  }
}
