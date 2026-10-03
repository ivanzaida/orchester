use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{repository::RepositoryLink, store::RepositoryStore};
use orchester_store::ProjectDb;

use super::{NewTaskProps, SaveAction, SaveArgs, SavedDraft};
use crate::{
  activity::{Activity, operations},
  background,
  messages::{self, Message},
  tasks::data::{TaskDraftSaveError, save_organized_to},
  workspace::OpenProject,
};

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

pub(super) fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let open = ctx.props::<NewTaskProps>().project.clone();
  let runners = ctx
    .use_context::<crate::runner::Runners>()
    .expect("root provides runners");
  ctx.future_action(
    move |(repo_root, target, details, (base, organization), stop): SaveArgs| {
      let runners = runners.clone();
      let (activity, spec) = (activity.clone(), operations::task_draft(&open, details.title.as_str()));
      async move {
        use orchester_core::task::organization::WorkflowStatus;
        // Only a person's change to Canceled stops a run, and only the run the person confirmed.
        let canceling = match target {
          crate::tasks::data::SaveTarget::Task(task)
            if organization.status() == WorkflowStatus::Canceled && base.status() != WorkflowStatus::Canceled =>
          {
            Some(task)
          }
          _ => None,
        };
        // Asking to confirm is not a failed save: it is not reported as one (`N1vwaM`).
        if let Some(task) = canceling
          && stop == crate::tasks::cancel_run::Stop::Unconfirmed
          && active_run(&repo_root, task).await?.is_some()
        {
          return Err(Message::new(messages::CANCEL_CONFIRM_REQUIRED));
        }
        let save = async move {
          if let Some(task) = canceling {
            crate::tasks::cancel_run::stop_named_run(&repo_root, task, stop, &runners)
              .await
              .map_err(|refused| refused.message())?;
          }
          let saved = background::run(move || {
            let mut db = ProjectDb::open(&repo_root).map_err(TaskDraftSaveError::Store)?;
            let task = save_organized_to(
              &mut db,
              target,
              details.clone(),
              (&base, &organization),
              orchester_core::time::Timestamp::now(),
            )?;
            if task.details() != &details {
              return Err(TaskDraftSaveError::DetailsMismatch);
            }
            let created = matches!(target, crate::tasks::data::SaveTarget::New(_));
            if created && task.status() != orchester_core::task::TaskStatus::Draft {
              return Err(TaskDraftSaveError::NotDraft(task.status()));
            }
            Ok(SavedDraft {
              details,
              organization,
              created: created.then(|| task.id()),
            })
          })
          .await?;
          saved.map_err(|error| {
            tracing::error!(%error, "cannot save the task draft");
            match error {
              TaskDraftSaveError::Store(error) => messages::store_error(&error),
              TaskDraftSaveError::NotDraft(_) => Message::new("tasks.new.noLongerDraft"),
              TaskDraftSaveError::DetailsMismatch => Message::new("tasks.new.savedDetailsDiffer"),
            }
          })
        };
        activity.track(spec, save).await
      }
    },
  )
}

/// The save was refused because the task has an active run that Canceled would stop: the form
/// asks before saving again with the cancellation confirmed.
pub(super) fn needs_cancel_confirmation(error: &Message) -> bool {
  error.key == messages::CANCEL_CONFIRM_REQUIRED
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
