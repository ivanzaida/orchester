//! Ready for review (`Task · Summary · ready for review` `F2LtFn`) and what merging makes of it (`Task ·
//! Summary · merge states` `kxMAN`). A run whose every step passed shows the Ready card ([`card`]) with
//! *Merge into main*, *Ask agent* and *Review PR*, then its stages, what changed and what verified it
//! ([`changed`]) and *Runs of this task*; beside them its details ([`details`]). Merging runs on the
//! project's shared operations (`ProjectJobs`, which Orchester's MCP server uses too), so it goes on
//! when the page closes: the button shows it under way. A merge that did not happen says why in place
//! of the Ready card, with the files that conflicted, and offers to merge again; a merged run says where
//! its work went and what was cleared after it.

mod card;
mod changed;
mod details;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::Element,
};
use orchester_workspace::tasks::integration_jobs::JobState;

use super::{TaskContext, arranged, runs_section, stages::stages};
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
    let parts: Vec<Element> = match (&props.detail.merge, &job.merge) {
      (Some(merge), _) => vec![card::merged(ctx, &props, merge), stages(run, None, now)]
        .into_iter()
        .chain(runs_section(ctx, &props))
        .collect(),
      (None, JobState::Failed(problem)) => card::not_merged(ctx, &props, problem, &job.conflicts),
      (None, _) => vec![
        card::ready(ctx, &props),
        stages(run, None, now),
        changed::columns(ctx, &props, read.as_ref()),
      ]
      .into_iter()
      .chain(runs_section(ctx, &props))
      .collect(),
    };
    let side = details::run_details(ctx, &props, run, now);
    arranged(parts, None, Some(side))
  }
}
