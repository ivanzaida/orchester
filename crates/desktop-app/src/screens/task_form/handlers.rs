//! What the form's actions start, and what the form does once they end: a created task opens Tasks with
//! its row highlighted, a saved edit becomes what is stored, a started run opens the task's
//! Conversation, and a task queued without a run keeps the form, locked, with Retry run.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::task::organization::TaskOrganization;
use orchester_workspace::tasks::data::SaveTarget;

use super::{
  actions::{RunAction, RunArgs, SaveAction, SaveArgs, Saved, Started},
  bar::{Handler, Handlers},
  form::{FormProps, TaskForm},
  state::{Phase, View},
};

impl TaskForm {
  pub(super) fn handlers(&self, props: &FormProps, view: &View, (save, run): (&SaveAction, &RunAction)) -> Handlers {
    let stored = self.stored_task(props);
    let args = view.ready.clone().map(|details| SaveArgs {
      repo_root: props.project.open.repo_root.clone(),
      target: stored.as_ref().map_or(SaveTarget::New(self.batch), |task| {
        SaveTarget::Task(task.id(), task.revision())
      }),
      details,
      organization: stored
        .as_ref()
        .map_or_else(TaskOrganization::default, |task| task.organization().clone()),
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

  /// Discard changes: the fields show what is stored again, and nothing stays kept.
  fn discard_handler(&self) -> Handler {
    let (fields, baseline) = (self.fields.clone(), self.baseline.clone());
    let (kept_at, confirming) = (self.kept_at.clone(), self.confirming_discard.clone());
    Arc::new(move || {
      if let Some(stored) = baseline.get_untracked() {
        fields.fill(&stored);
      }
      kept_at.set(None);
      confirming.set(false);
    })
  }

  /// Once a save stored the task: a new one opens Tasks with its row highlighted; an edit is what is
  /// stored from now on.
  pub(super) fn follow_save(&self, ctx: &mut Ctx, save: &SaveAction, props: &FormProps) {
    let (feed, nav, new) = (props.project.feed.clone(), self.nav.clone(), props.subject.is_new());
    let (saved, kept_at) = (self.saved.clone(), self.kept_at.clone());
    let (kept, written) = (self.kept.clone(), self.written.clone());
    ctx.watch(&save.state(), move |state| {
      let Some(Saved(task)) = state.data.clone().filter(|_| state.is_fulfilled()) else {
        return;
      };
      feed.saved();
      if new {
        Self::forget_kept(&kept, &written);
        nav.tasks(Some(task.key().clone()));
      } else {
        kept_at.set(None);
        saved.set(Some(Saved(task)));
      }
    });
  }

  /// Once Create & run or Retry run ended: a started run opens the task's Conversation; a task queued
  /// without a run keeps the form, which says why and offers Retry run.
  pub(super) fn follow_run(&self, ctx: &mut Ctx, run: &RunAction, props: &FormProps) {
    let (feed, nav, queued) = (props.project.feed.clone(), self.nav.clone(), self.queued.clone());
    let (kept, written) = (self.kept.clone(), self.written.clone());
    ctx.watch(&run.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(problems) = state.error.as_ref().filter(|_| state.is_rejected()) {
        if let Some(task) = &problems.queued {
          Self::forget_kept(&kept, &written);
          queued.set(Some(task.clone()));
          feed.saved();
        }
        return;
      }
      if let Some(Started(key)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        Self::forget_kept(&kept, &written);
        feed.saved();
        nav.started(&key);
      }
    });
  }
}
