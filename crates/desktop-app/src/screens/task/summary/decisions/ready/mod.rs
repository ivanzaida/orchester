//! Ready for review (`Task · Summary · ready for review` `F2LtFn`) and what merging makes of it (`Task ·
//! Summary · merge states` `kxMAN`). A run whose every step passed shows the Ready card ([`card`]) with
//! *Merge into main*, *Ask agent* and *Review PR*, then its stages, what changed and what verified it
//! ([`changed`]) and *Runs of this task*; beside them *Run details*. Merging runs on the
//! project's shared operations (`ProjectJobs`, which Orchester's MCP server uses too), so it goes on
//! when the page closes: the button shows it under way. A merge that did not happen says why in place
//! of the Ready card, with the files that conflicted, and offers to merge again; a merged run says where
//! its work went and what was cleared after it.

mod card;
mod changed;
mod local;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::Element,
};
use orchester_workspace::tasks::integration_jobs::JobState;

use super::{super::run_details::run_details, TaskContext, arranged, runs_section, stages::stages};
use crate::services::Services;

pub struct ReadyForReview;

impl Component for ReadyForReview {
  type Props = TaskContext;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let read = changed::read(ctx, &props);
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let job = props.jobs.run(run.id());
    // A merge git made shows as soon as it is made, also while the task is not read again with it.
    let made = props.detail.merge.as_ref().or(job.merged.as_ref());
    let mut parts: Vec<Element> = match (made, &job.merge) {
      (Some(merge), JobState::Failed(problem)) if props.detail.merge.is_none() => {
        card::not_merged(ctx, &props, run, (problem, Some(merge)), &job.conflicts)
      }
      (Some(merge), _) => vec![card::merged(ctx, &props, merge), stages(run, None, now)]
        .into_iter()
        .chain(runs_section(ctx, &props))
        .collect(),
      (None, JobState::Failed(problem)) => card::not_merged(ctx, &props, run, (problem, None), &job.conflicts),
      (None, _) => vec![
        card::ready(ctx, &props, run),
        stages(run, None, now),
        changed::columns(ctx, &props, read.as_ref()),
      ]
      .into_iter()
      .chain(runs_section(ctx, &props))
      .collect(),
    };
    if let Some(reminder) = local::reminder(ctx, &props) {
      parts.insert(0, reminder);
    }
    let side = run_details(ctx, &props, run);
    arranged(ctx, parts, None, Some(side))
  }
}
