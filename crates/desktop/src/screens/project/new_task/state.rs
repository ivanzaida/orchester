//! What the form holds compared with what is stored, and what can be pressed.

use orchester_core::limits::ProjectLimits;

use super::{Editing, FormState, NewTaskForm, RunAction, SaveAction};
use crate::tasks::form::{FormErrors, TaskForm};

impl NewTaskForm {
  pub(super) fn state(
    &self,
    save: &SaveAction,
    run: &RunAction,
    separator: &str,
    (limits, editing): (ProjectLimits, Option<&Editing>),
    recovered: bool,
  ) -> FormState {
    let criteria: Vec<String> = self
      .criteria
      .get()
      .iter()
      .map(|criterion| criterion.text.get())
      .collect();
    let (title, brief, budget, time_limit) = (
      self.title.get(),
      self.brief.get(),
      self.budget.get(),
      self.time_limit.get(),
    );
    let form = TaskForm {
      title: &title,
      brief: &brief,
      criteria: &criteria,
      pipeline: self.pipeline.get().0,
      budget: &budget,
      time_limit: &time_limit,
      separator,
      max_budget: limits.max_run_budget(),
    };
    let saving = save.state().get();
    let (mut ready, errors) = match form.details() {
      Ok(details) => (Some(details), FormErrors::default()),
      Err(errors) => (None, errors),
    };
    if self.organization.snapshot().is_err() {
      ready = None;
    }
    let starting = run.state().get();
    let queued_task = self.queued_task.get();
    let queued = queued_task.is_some();
    let stored = saving
      .data
      .as_ref()
      .map(|saved| &saved.details)
      .or_else(|| editing.map(|editing| editing.task.details()));
    FormState {
      errors,
      missing: form.missing(),
      saved: !queued
        && ready.as_ref().is_some_and(|details| stored == Some(details))
        && self.organization.snapshot().is_ok_and(|value| {
          saving
            .data
            .as_ref()
            .map(|saved| &saved.organization)
            .or_else(|| editing.map(|editing| editing.task.organization()))
            == Some(&value)
        }),
      editing: editing.is_some(),
      // A task this form queued has no run until the runner starts one; until then it keeps
      // Retry run.
      started: editing.is_some_and(|editing| editing.has_run),
      queued,
      recovered,
      ready,
      saving: saving.is_pending(),
      // Only a save that failed last shows its error; asking to confirm Canceled is not one.
      save_error: saving
        .error
        .clone()
        .filter(|error| saving.is_rejected() && !super::persistence::needs_cancel_confirmation(error)),
      starting: starting.is_pending(),
      retrying: self.retrying.get(),
      starting_pipeline: starting
        .is_pending()
        .then(|| self.starting_pipeline.get().map(|held| held.0))
        .flatten(),
      queued_task,
      run_problems: starting.error.clone().filter(|_| !starting.is_pending()),
    }
  }
}
