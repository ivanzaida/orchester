//! *Try again* on the attempt page: merges the run as the task page's *Merge into <base>* does, on the project's shared
//! operations.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::tasks::{
  integration_jobs::{JobState, MergeArgs},
  merge_attempts::merges::MergeRun,
};

use crate::{project::ShownProject, services::Services, ui::button::ActionButton};

/// *Try again*: under way while the run merges, waiting while another run of the project merges, unavailable without
/// the branch it merges into.
pub(super) fn try_again(ctx: &mut Ctx, project: &ShownProject, button: ActionButton, run: &MergeRun) -> Element {
  let Some(branch) = run.base.clone() else {
    return button.unavailable(ctx.t("merges.action.noBranch")).into();
  };
  let jobs = project.feed.job_state.get();
  if jobs.run(run.run).merge == JobState::Running {
    return button.loading().into();
  }
  if jobs.merging().is_some_and(|other| other != run.run) {
    return button.unavailable(ctx.t("merges.action.anotherMerge")).into();
  }
  let services = Services::of(ctx);
  let feed = project.feed.clone();
  let args = MergeArgs {
    open: project.open.clone(),
    task: run.task,
    key: run.key.to_string(),
    number: run.number.get(),
    branch: branch.as_str().to_owned(),
    run: run.run,
    worktrees: services.worktree_remover(&project.open),
    programs: services.programs,
    activity: services.activity,
    separator: ctx.t("task.listSeparator"),
    changed: Arc::new(move || feed.saved()),
  };
  let (job, vault) = (project.feed.jobs.of(run.run), services.vault);
  // A merge that does not begin (another one took the project meanwhile) leaves the job's state as it is.
  let start = move || {
    if lurq::app::events::is_synthetic_input() {
      job.start_synthetic_merge(args.clone(), vault.clone());
    } else {
      job.start_merge(args.clone());
    }
  };
  button.on_click(start).into()
}
