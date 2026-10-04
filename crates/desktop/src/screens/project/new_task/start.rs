//! Queuing a form once and retrying its run under the same key.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::task::TaskDetails;

use super::{NewTaskForm, QueuedTask, RunAction, RunArgs, RunProblems, StartedTask, StartingPipeline};
use crate::{
  background,
  runner::Runners,
  sharing::feed::ServerFeed,
  tasks::{
    data::{PipelineChoice, SaveTarget},
    run::{StartOutcome, start_messages},
  },
  ui::form::Submit,
  workspace::OpenProject,
};

impl NewTaskForm {
  /// Queue the selected version, or retry the already queued task under the same run key.
  pub(super) fn run_handler(
    &self,
    run: &RunAction,
    (project, target, pipelines): (&OpenProject, SaveTarget, Option<&Vec<PipelineChoice>>),
    ready: Option<TaskDetails>,
    queued: Option<QueuedTask>,
  ) -> Submit {
    let run = run.clone();
    let retrying = self.retrying.clone();
    let starting_pipeline = self.starting_pipeline.clone();
    let args = match queued {
      Some(queued) => RunArgs::Retry { queued },
      None => RunArgs::New {
        repo_root: project.repo_root.clone(),
        target,
        details: Box::new(ready?),
        // Invalid organization is explained beside its fields and prevents Run.
        organization: Box::new(self.organization.change()?),
        pipeline: Arc::new(
          pipelines?
            .iter()
            .find(|choice| Some(choice.id) == self.pipeline.get().0)?
            .clone(),
        ),
      },
    };
    let is_retry = matches!(&args, RunArgs::Retry { .. });
    let pending_pipeline = match &args {
      RunArgs::New { pipeline, .. } => pipeline.clone(),
      RunArgs::Retry { queued, .. } => queued.pipeline.clone(),
    };
    Some(Arc::new(move || {
      retrying.set(is_retry);
      starting_pipeline.set(Some(StartingPipeline(pending_pipeline.clone())));
      run.run(args.clone());
    }))
  }
}

pub(super) fn run_action(ctx: &mut Ctx, queued_task: Signal<Option<QueuedTask>>) -> RunAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  let servers = ServerFeed::of(ctx).servers;
  let root = ctx.props::<super::NewTaskProps>().project.repo_root.clone();
  ctx.future_action(move |args: RunArgs| {
    let (runners, servers, root) = (runners.clone(), servers.clone(), root.clone());
    let queued_state = queued_task.clone();
    async move {
      let queued = match args {
        RunArgs::New {
          repo_root,
          target,
          details,
          organization,
          pipeline,
        } => {
          let selected_version = pipeline.version;
          let servers = servers.clone();
          let queued = background::run(move || {
            crate::tasks::run::queue_organized_in(
              &repo_root,
              &servers,
              target,
              *details,
              (&organization.0, &organization.1),
              selected_version,
            )
          })
          .await
          .map_err(|error| RunProblems {
            messages: vec![error].into(),
            queued: None,
          })?;
          let saved = queued.map_err(|failure| {
            tracing::warn!(?failure, "the task was not queued for a run");
            RunProblems {
              messages: failure.messages().into(),
              queued: None,
            }
          })?;
          let queued = QueuedTask {
            task: saved.task.id(),
            outcome: StartOutcome::ConfirmedNotStarted,
            pipeline: Arc::new(saved.pipeline),
          };
          queued_state.set(Some(queued.clone()));
          queued
        }
        RunArgs::Retry { queued } => queued,
      };
      let task = queued.task;
      let key = task.first_run_key();
      // Work on a shared task starts only under the person's claim.
      servers
        .claim_for_work(&root, task)
        .await
        .map_err(|refusal| RunProblems {
          messages: vec![refusal.message()].into(),
          queued: Some(queued.clone()),
        })?;
      runners.start_run(key, task).await.map_err(|error| {
        tracing::warn!(%error, "the start request failed");
        RunProblems {
          messages: start_messages(&error).into(),
          queued: Some(QueuedTask {
            task,
            outcome: queued.outcome.after(&error),
            pipeline: queued.pipeline,
          }),
        }
      })?;
      Ok(StartedTask(task))
    }
  })
}
