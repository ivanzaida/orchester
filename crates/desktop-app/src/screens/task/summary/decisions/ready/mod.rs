//! A run whose every step passed waits to be merged (ORC-731; `Review · redesign`, Row 4 `XHQpC`), and what merging
//! makes of it (`Task · Summary · merge states` `kxMAN`). The top card of the Summary ([`card`]) is *Ready to merge*
//! (01i `k9GBy`) with *Merge*, *Review changes*, which opens the run's review page, and *Close without
//! merging*; once a merge did not land it turns to the warning card (01j `k4C0Es`): the outcome, what stopped it in
//! one line, the attempt page's primary action and *Open in Review* (`crate::screens::task::merge`). The run's pull
//! request is created from the header's *More actions* ([`pull_request`]); once it has one, the card says so and
//! *Review PR* opens it. Merging and creating a pull request run on the project's shared operations (`ProjectJobs`,
//! which Orchester's MCP server uses too), so they go on when the page closes: the buttons show them under way. While
//! a merge of the base branch waits in the run's worktree for the person to finish it, as the task's frame read it
//! (`merge::worktree`), that is shown instead ([`resolving`]); a merged run says where its work went and what was
//! cleared after it.

mod card;
pub(super) mod changed;
mod external;
mod local;
mod pull_request;
mod resolving;
mod unpushed;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::id::RunId;
use orchester_workspace::tasks::{integration_jobs::JobState, resolve::Worktree};

pub(super) use self::card::frame;
pub use self::pull_request::PullRequestState;
use super::{TaskContext, arranged};
use crate::screens::task::{
  PageState,
  merge::{self, Merge},
};

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
    // Read on every render, before the parts only some states build: lurq keeps futures by their order. Whether a
    // pull request can be created is read once the checkout's own reads settled: they hold the checkout, and the
    // remote's read runs git in it.
    let (external, settled) = external::notice(ctx, &props);
    let (reminder, reminded) = local::reminder(ctx, &props);
    let readiness = pull_request::read(ctx, &props, settled && reminded);
    let pull = PageState::of(ctx).pull_request.read(readiness);
    let in_merge = props.worktree.as_ref().and_then(|worktree| worktree.merge.as_ref());
    let job = props.jobs.run(run.id());
    // A merge git made shows as soon as it is made, also while the task is not read again with it; one it could not
    // record is not merged yet, and the top card's merge records it.
    let unrecorded = props.detail.merge.is_none() && matches!(job.merge, JobState::Failed(_));
    let made = props
      .detail
      .merge
      .as_ref()
      .or(job.merged.as_ref().filter(|_| !unrecorded));
    let mut parts: Vec<Element> = match (made, Merge::of(&props)) {
      (None, _) if let Some(resolution) = in_merge => resolving::resolving(ctx, &props, run, (resolution, &pull)),
      (Some(merge), _) => vec![card::merged(ctx, &props, merge)],
      (None, Some(merge)) => {
        let card = card::Shown {
          merge,
          run,
          pull: &pull,
        };
        vec![external.unwrap_or_else(|| card::top(ctx, &props, &card, read.as_ref()))]
      }
      (None, None) => Vec::new(),
    };
    if let Some(reminder) = reminder {
      parts.insert(0, reminder);
    }
    arranged(parts)
  }
}

/// *Create pull request* and *Open as draft* for the header's *More actions*, which `menu` opens, while the task's
/// latest run waits to be merged; read on every render of the header, in the same place: lurq keeps futures by their
/// order. Whether one can be created is read while the menu is open.
pub fn pull_request_rows(ctx: &mut Ctx, task: &TaskContext, menu: &Signal<bool>) -> Vec<Element> {
  let waits = Merge::of(task).is_some();
  let open = menu.get();
  let readiness = pull_request::read(ctx, task, waits && open);
  if !(waits && open) {
    return Vec::new();
  }
  let read = PageState::of(ctx).pull_request.read(readiness);
  pull_request::menu_rows(ctx, task, &read, menu)
}

/// `run`'s worktree and the merge waiting in it for the person to resolve it, read again when the task or its
/// resolving changes and at each `look` while one waits; `None` while it is first read.
pub(super) fn in_worktree(ctx: &mut Ctx, task: &TaskContext, (run, look): (RunId, u64)) -> Option<Worktree> {
  let job = task.jobs.run(run);
  let again = ((task.project.feed.revision.get(), job.resolve.ended), look);
  merge::worktree::read(ctx, &task.project.open, (run, !job.resolving()), again)
}
