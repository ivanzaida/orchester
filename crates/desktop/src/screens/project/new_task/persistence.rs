use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{repository::RepositoryLink, store::RepositoryStore};
use orchester_store::ProjectDb;

use super::{NewTaskProps, SaveAction, SaveArgs, SavedDraft};
use crate::{
  activity::{Activity, operations},
  background,
  messages::{self, Message},
  sharing::{Servers, feed::ServerFeed},
  tasks::{
    conflict::{Edit, EditConflict, EditSaved, save_edit},
    data::{SaveTarget, TaskDraftSaveError},
    writer::TaskWriter,
  },
  workspace::OpenProject,
};

/// The save met a newer version of the task: the form shows the conflict instead of an error.
pub(super) const CONFLICT: &str = "tasks.conflict.notSaved";

/// The project's repository link, once read. Without it the form shows the local folder, which
/// is the repository either way, so a failed read is logged and the folder shown.
pub(super) fn repository_link(ctx: &mut Ctx, project: &OpenProject) -> Option<Arc<RepositoryLink>> {
  /// The link as the future holds it.
  #[derive(Clone, PartialEq)]
  struct Linked(Option<Arc<RepositoryLink>>);
  impl lurq::app::component::DevtoolsInspectable for Linked {}

  let load = ctx.future(project.repo_root.clone(), |repo_root| async move {
    background::run(move || ProjectDb::open(&repo_root).and_then(|db| db.repository_link()))
      .await?
      .map(|link| Linked(link.map(Arc::new)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's repository link");
        messages::store_error(&error)
      })
  });
  load.state().get().data.and_then(|linked| linked.0)
}

pub(super) fn save_action(ctx: &mut Ctx, conflict: Signal<Option<EditConflict>>) -> SaveAction {
  let activity = Activity::of(ctx);
  let open = ctx.props::<NewTaskProps>().project.clone();
  let runners = ctx
    .use_context::<crate::runner::Runners>()
    .expect("root provides runners");
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |args: SaveArgs| {
    let (runners, servers, conflict) = (runners.clone(), servers.clone(), conflict.clone());
    let (activity, spec) = (
      activity.clone(),
      operations::task_draft(&open, args.details.title.as_str()),
    );
    async move {
      use orchester_core::task::organization::WorkflowStatus;
      let (base, organization) = &args.organization;
      // Only a person's change to Canceled stops a run, and only the run the person confirmed.
      let canceling = match args.target {
        SaveTarget::Task(task, _)
          if organization.status() == WorkflowStatus::Canceled && base.status() != WorkflowStatus::Canceled =>
        {
          Some(task)
        }
        _ => None,
      };
      // Asking to confirm is not a failed save: it is not reported as one (`N1vwaM`).
      if let Some(task) = canceling
        && args.stop == crate::tasks::cancel_run::Stop::Unconfirmed
        && active_run(&args.repo_root, task).await?.is_some()
      {
        return Err(Message::new(messages::CANCEL_CONFIRM_REQUIRED));
      }
      let save = async move {
        if let Some(task) = canceling {
          crate::tasks::cancel_run::stop_named_run(&args.repo_root, task, args.stop, &runners)
            .await
            .map_err(|refused| refused.message())?;
        }
        let saved = background::run(move || store(&args, &servers)).await?;
        match saved {
          Ok(Stored::Saved(draft)) => Ok(*draft),
          Ok(Stored::Conflict(found)) => {
            conflict.set(Some(*found));
            Err(Message::new(CONFLICT))
          }
          Err(error) => {
            tracing::error!(%error, "cannot save the task draft");
            Err(match error {
              TaskDraftSaveError::Store(error) => messages::store_error(&error),
              TaskDraftSaveError::Write(error) => error.message(),
              TaskDraftSaveError::NotDraft(_) => Message::new("tasks.new.noLongerDraft"),
              TaskDraftSaveError::DetailsMismatch => Message::new("tasks.new.savedDetailsDiffer"),
              TaskDraftSaveError::Changed(_) => Message::new("tasks.conflict.changedMeanwhile"),
            })
          }
        }
      };
      activity.track(spec, save).await
    }
  })
}

/// How a save of the form ended.
enum Stored {
  Saved(Box<SavedDraft>),
  Conflict(Box<EditConflict>),
}

/// Saves the form's `args`: Create as a new draft under the form's key, or the edit of a task,
/// merged into a task changed meanwhile when nothing both changed conflicts.
fn store(args: &SaveArgs, servers: &Servers) -> Result<Stored, TaskDraftSaveError> {
  let mut writer = TaskWriter::open(&args.repo_root, servers)?;
  let (base, organization) = (&args.organization.0, &args.organization.1);
  let now = orchester_core::time::Timestamp::now();
  let (task, details, created) = match args.target {
    SaveTarget::New(_) => {
      let task = writer
        .save_organized(args.target, args.details.clone(), (base, organization), now)?
        .saved()
        .ok_or(TaskDraftSaveError::DetailsMismatch)?;
      if task.status() != orchester_core::task::TaskStatus::Draft {
        return Err(TaskDraftSaveError::NotDraft(task.status()));
      }
      (task, args.details.clone(), true)
    }
    SaveTarget::Task(id, read) => {
      let edit = Edit {
        task: id,
        read,
        base: &args.base,
        details: args.details.clone(),
        organization: (base, organization),
      };
      match save_edit(&mut writer, edit, now)? {
        EditSaved::Saved { task, details } => (task, details, false),
        EditSaved::Conflict(conflict) => return Ok(Stored::Conflict(Box::new(conflict))),
      }
    }
  };
  if task.details() != &details {
    return Err(TaskDraftSaveError::DetailsMismatch);
  }
  Ok(Stored::Saved(Box::new(SavedDraft {
    merged: details != args.details || task.organization() != organization,
    details,
    organization: task.organization().clone(),
    created: created.then(|| task.id()),
    revision: task.revision(),
  })))
}

/// The save was refused because the task has an active run that Canceled would stop: the form
/// asks before saving again with the cancellation confirmed.
pub(super) fn needs_cancel_confirmation(error: &Message) -> bool {
  error.key == messages::CANCEL_CONFIRM_REQUIRED
}

/// The save met a newer version of the task, which the form shows in place of an error.
pub(super) fn met_newer_version(error: &Message) -> bool {
  error.key == CONFLICT
}

/// The run of `task` that has not ended, if any.
async fn active_run(
  root: &std::path::Path,
  task: orchester_core::id::TaskId,
) -> Result<Option<orchester_core::id::RunId>, Message> {
  let root = root.to_owned();
  background::run(move || ProjectDb::open(&root).and_then(|db| db.active_task_run(task)))
    .await?
    .map_err(|error| messages::store_error(&error))
}
