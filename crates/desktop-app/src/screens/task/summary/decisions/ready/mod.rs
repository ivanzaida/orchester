//! Ready for review (`Task · Summary · ready for review` `F2LtFn`) and what merging makes of it (`Task ·
//! Summary · merge states` `kxMAN`). A run whose every step passed shows the Ready card ([`card`]) with
//! *Merge into main*, *Create pull request* ([`pull_request`]), *Ask agent* and *Review PR*, then its stages,
//! what changed and what verified it ([`changed`]) and *Runs of this task*; beside them *Run details*. Merging
//! and creating a pull request run on the project's shared operations (`ProjectJobs`, which Orchester's MCP
//! server uses too), so they go on when the page closes: the buttons show them under way. A merge that did not
//! happen says why in place of the Ready card, with the files that conflicted, how to resolve them — in the person's
//! editor or by an agent of the run ([`resolve`]) — and offers to merge again; while a merge waits in the run's
//! worktree for the person to resolve it, that is shown instead ([`resolving`]); a merged run says where its work
//! went and what was cleared after it.

mod card;
mod changed;
mod external;
mod local;
mod pull_request;
mod resolve;
mod resolving;

use std::time::Duration;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Interval},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  tasks::{
    integration_jobs::JobState,
    resolve::{self as worktree, Worktree},
  },
};

use self::{
  pull_request::PullRequestRead,
  resolve::{Asking, Offer, StepChoice},
};
use super::{
  super::run_details::run_details, TaskContext, action::decision_action, arranged, runs_section, stages::stages,
};
use crate::services::Services;

/// How often a merge waiting in the run's worktree is read again while the person resolves it.
const LOOK: Duration = Duration::from_secs(2);

pub struct ReadyForReview {
  /// *Open as draft*: on until the person turns it off (rule 7).
  draft: Signal<bool>,
  /// Counts while a merge waits in the run's worktree, which is read again then: the person resolves it in an editor,
  /// which tells Orchester nothing.
  look: Signal<u64>,
  /// Ticks [`Self::look`]; it runs only while a merge waits in the worktree.
  looking: Interval,
  /// The step *Ask an agent to resolve* returns the run to.
  step: Signal<StepChoice>,
  /// The return under way.
  asked: Signal<Option<Asking>>,
}

impl Component for ReadyForReview {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    let look = ctx.signal(0_u64);
    let next = look.clone();
    let looking = ctx.create_interval(LOOK, move || next.update(|tick| *tick += 1));
    Self {
      draft: ctx.signal(true),
      look,
      looking,
      step: ctx.signal(StepChoice(None)),
      asked: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let read = changed::read(ctx, &props);
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    // Read on every render, before the parts only some states build: lurq keeps futures by their order. Whether a
    // pull request can be created is read once the checkout's own reads settled: they hold the checkout, and the
    // remote's read runs git in it.
    let (external, settled) = external::notice(ctx, &props);
    let (reminder, reminded) = local::reminder(ctx, &props);
    let pull = PullRequestRead {
      readiness: pull_request::read(ctx, &props, settled && reminded),
      draft: self.draft.clone(),
    };
    let worktree = in_worktree(ctx, &props, (run.id(), self.look.get()));
    let in_merge = worktree.as_ref().and_then(|worktree| worktree.merge.as_ref());
    if in_merge.is_some() {
      self.looking.start();
    } else {
      self.looking.stop();
    }
    let action = decision_action::<Asking>(ctx, props.project.open.clone());
    let now = Services::of(ctx).clock.now();
    let job = props.jobs.run(run.id());
    // A merge git made shows as soon as it is made, also while the task is not read again with it.
    let made = props.detail.merge.as_ref().or(job.merged.as_ref());
    let mut parts: Vec<Element> = match (made, &job.merge) {
      (None, _) if let Some(resolution) = in_merge => resolving::resolving(ctx, &props, run, (resolution, &pull))
        .into_iter()
        .chain([stages(run, None, now)])
        .chain(runs_section(ctx, &props))
        .collect(),
      (Some(merge), JobState::Failed(problem)) if props.detail.merge.is_none() => {
        card::not_merged(ctx, &props, run, (problem, Some(merge)), (&job.conflicts, &pull), None)
      }
      (Some(merge), _) => vec![card::merged(ctx, &props, merge), stages(run, None, now)]
        .into_iter()
        .chain(runs_section(ctx, &props))
        .collect(),
      (None, JobState::Failed(problem)) => {
        let offer = (!job.conflicts.is_empty()).then(|| {
          Offer {
            task: &props,
            run,
            worktree: worktree.as_ref(),
            conflicts: &job.conflicts,
            step: &self.step,
            asked: &self.asked,
            action: &action,
          }
          .render(ctx)
        });
        card::not_merged(ctx, &props, run, (problem, None), (&job.conflicts, &pull), offer)
      }
      (None, _) => vec![
        external.unwrap_or_else(|| card::ready(ctx, &props, run, &pull)),
        stages(run, None, now),
        changed::columns(ctx, &props, read.as_ref()),
      ]
      .into_iter()
      .chain(runs_section(ctx, &props))
      .collect(),
    };
    if let Some(reminder) = reminder {
      parts.insert(0, reminder);
    }
    let linked = crate::screens::task::sharing::linked(ctx, &props);
    let section = crate::screens::task::sharing::section(ctx, &props, (linked.as_ref(), None));
    let side = run_details(ctx, &props, run, section);
    arranged(ctx, parts, None, Some(side))
  }
}

/// What [`in_worktree`] read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InWorktree(Worktree);

impl lurq::app::component::DevtoolsInspectable for InWorktree {}

/// `run`'s worktree and the merge waiting in it for the person to resolve it, read again when the task or its
/// resolving changes and at each `look` while one waits; `None` while it is first read.
fn in_worktree(ctx: &mut Ctx, task: &TaskContext, (run, look): (RunId, u64)) -> Option<Worktree> {
  let services = Services::of(ctx);
  let root = task.project.open.repo_root.clone();
  let job = task.jobs.run(run);
  let idle = !job.resolving();
  let key = (
    root,
    (run, idle),
    (task.project.feed.revision.get(), job.resolve.ended, look),
  );
  ctx
    .future(key, move |(root, (run, idle), _)| {
      let programs = services.programs.clone();
      async move {
        background::run(move || worktree::read(&programs, &root, (run, idle)))
          .await
          .map_err(|_| ())?
          .map(InWorktree)
          .map_err(|message| {
            tracing::warn!(?message, "cannot read whether a merge waits in the run's worktree");
          })
      }
    })
    .state()
    .get()
    .data
    .map(|read| read.0)
}
