//! What the form's actions start, and what the form does once they end: a created task opens Tasks with
//! its row highlighted, a saved edit becomes the version further edits are made on, a save that met a
//! newer version where both changed a field shows the fields to choose between, a started run opens the
//! task's Conversation, and a task queued without a run keeps the form, locked, with Retry run.
//!
//! An action asked for again while it is under way would replace the request in flight
//! (`FutureAction::run`), so every press is ignored while its action is pending.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::task::organization::TaskOrganization;
use orchester_workspace::{modules::ModuleChoice, tasks::data::SaveTarget};

use super::{
  actions::{RunAction, RunArgs, SaveAction, SaveArgs, SaveOutcome, Saved, Started},
  bar::{Handler, Handlers},
  base::{Base, Newer, Written},
  conflict::{Choices, Conflict},
  form::{FormProps, TaskForm},
  state::{Phase, View},
  values::Values,
};

/// Runs `action` with `args`, unless it is under way; a new attempt shows its own failure, if any.
fn press<A: Clone + Send + Sync + 'static, T, E>(
  action: &lurq::app::ctx::FutureAction<A, T, E>,
  args: A,
  dismissed: &Signal<bool>,
) -> Handler
where
  T: lurq::core::SignalValue + Clone + PartialEq + Send + Sync + 'static,
  E: lurq::core::SignalValue + Clone + PartialEq + Send + Sync + 'static,
{
  let (action, dismissed) = (action.clone(), dismissed.clone());
  Arc::new(move || {
    if !action.state().get_untracked().is_pending() {
      dismissed.set(false);
      action.run(args.clone());
    }
  })
}

impl TaskForm {
  pub(super) fn handlers(&self, props: &FormProps, view: &View, (save, run): (&SaveAction, &RunAction)) -> Handlers {
    let base = self.base.get_untracked();
    let made_on = base
      .as_ref()
      .and_then(|base| Some((base.task.clone()?, base.revision?, base.values.clone())));
    let separator = props.separator.clone();
    let organization = made_on
      .as_ref()
      .map_or_else(TaskOrganization::default, |(task, ..)| task.organization().clone());
    let base_values = base.map(|base| base.values).unwrap_or_default();
    let edited = view.values.organization(&base_values, &organization);
    let args = view.ready.clone().map(|details| SaveArgs {
      repo_root: props.project.open.repo_root.clone(),
      // On the version the edits were made on: a change made meanwhile is met, never overwritten.
      target: made_on
        .as_ref()
        .map_or(SaveTarget::New(self.batch), |(task, revision, _)| {
          SaveTarget::Task(task.id(), *revision)
        }),
      base: made_on
        .as_ref()
        .and_then(|(_, _, values)| values.task_form(&separator).details().ok()),
      details,
      organization: (organization.clone(), edited.clone()),
      module: view.module,
    });
    let (ask, keep) = (self.confirming_discard.clone(), self.confirming_discard.clone());
    Handlers {
      save: args.clone().map(|args| press(save, args, &self.dismissed)),
      run: self.run_handler(view, run, args),
      ask_discard: Arc::new(move || ask.set(true)),
      keep_editing: Arc::new(move || keep.set(false)),
      discard: self.discard_handler(),
      save_chosen: self.save_chosen_handler(props, view, save),
      back_to_editing: self.back_to_editing_handler(),
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
    let (retrying, run, dismissed) = (self.retrying.clone(), run.clone(), self.dismissed.clone());
    Some(Arc::new(move || {
      if !run.state().get_untracked().is_pending() {
        dismissed.set(false);
        retrying.set(retry);
        run.run(request.clone());
      }
    }))
  }

  /// Save chosen versions: the newer version with every field the person changed, a conflicting one as
  /// chosen, saved on the newer revision, so a change saved after it is met again. `None` until every
  /// field is chosen.
  fn save_chosen_handler(&self, props: &FormProps, view: &View, save: &SaveAction) -> Option<Handler> {
    let conflict = view.conflict.as_ref()?;
    let details = conflict.resolved(&view.choices)?;
    let current = &conflict.0.current;
    let base_values = self.base.get_untracked().map(|base| base.values).unwrap_or_default();
    let organization = current.organization().clone();
    let edited = view.values.organization(&base_values, &organization);
    let args = SaveArgs {
      repo_root: props.project.open.repo_root.clone(),
      target: SaveTarget::Task(current.id(), current.revision()),
      base: Some(current.details().clone()),
      details,
      organization: (organization, edited),
      module: view.module,
    };
    Some(press(save, args, &self.dismissed))
  }

  /// Back to editing: the edits stay as they are, on the version they were made on; the next save meets
  /// the newer one again.
  fn back_to_editing_handler(&self) -> Handler {
    let conflict = self.conflict.clone();
    Arc::new(move || conflict.set(None))
  }

  /// Discard changes: the fields show the version they were made on again (New task: empty), and
  /// nothing stays kept.
  fn discard_handler(&self) -> Handler {
    let (fields, base) = (self.fields.clone(), self.base.clone());
    let (kept_at, confirming) = (self.kept_at.clone(), self.confirming_discard.clone());
    let (kept, written) = (self.kept.clone(), self.written.clone());
    Arc::new(move || {
      let base = base.get_untracked();
      let module = base.as_ref().and_then(Base::module);
      let values = base.map_or_else(Values::default, |base| base.values);
      kept.remove();
      *written.lock() = Written::Nothing;
      fields.fill(&values);
      fields.module.set(ModuleChoice(module));
      kept_at.set(None);
      confirming.set(false);
    })
  }

  /// Once a save ended: a new task opens Tasks with its row highlighted; an edit, merged with a version
  /// saved meanwhile where they changed different fields, is shown and made the version further edits
  /// are made on; fields both changed are shown to choose between. Acts only on a save that succeeded
  /// (`is_fulfilled`): a failed one keeps the data of an earlier success.
  pub(super) fn follow_save(&self, ctx: &mut Ctx, save: &SaveAction, props: &FormProps) {
    let (feed, nav, new, separator) = (
      props.project.feed.clone(),
      self.nav.clone(),
      props.subject.is_new(),
      props.separator.clone(),
    );
    let (fields, base, newer, kept_at) = (
      self.fields.clone(),
      self.base.clone(),
      self.newer.clone(),
      self.kept_at.clone(),
    );
    let (conflict, choices, dismissed) = (self.conflict.clone(), self.choices.clone(), self.dismissed.clone());
    let (kept, written) = (self.kept.clone(), self.written.clone());
    ctx.watch(&save.state(), move |state| {
      let Some(outcome) = state.data.clone().filter(|_| state.is_fulfilled()) else {
        return;
      };
      feed.saved();
      let task = match outcome {
        SaveOutcome::Conflict(met) => {
          // The newer version is known: leaving the choice does not meet it again until the next save.
          newer.set(Some(Newer(met.current.clone())));
          choices.set(Choices::default());
          conflict.set(Some(Conflict(met)));
          return;
        }
        SaveOutcome::Stored(task) => task,
      };
      if new && base.get_untracked().is_none_or(|base| base.task.is_none()) {
        Self::forget_kept(&kept, &written);
        nav.tasks(task.key().clone());
        return;
      }
      let stored = Base::of(&task, &separator);
      kept.remove();
      *written.lock() = Written::Nothing;
      fields.fill(&stored.values);
      fields.module.set(ModuleChoice(stored.module()));
      base.set(Some(stored));
      dismissed.set(true);
      conflict.set(None);
      newer.set(None);
      kept_at.set(None);
    });
  }

  /// Once Create & run or Retry run ended: a started run opens the task's Conversation; a task queued
  /// without a run keeps the form, which says why and offers Retry run; a draft saved without being
  /// queued is the one the form edits from now on, never a second task.
  pub(super) fn follow_run(&self, ctx: &mut Ctx, run: &RunAction, props: &FormProps) {
    let separator = props.separator.clone();
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
      if let Some(Started(key, began)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        Self::forget_kept(&kept, &written);
        nav.started(&key, began);
      }
    });
  }
}
