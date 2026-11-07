//! The task frame's helpers: the readers of the latest run's merge and its pull request's CI, the run controls it
//! provides, and the notice of a reading that failed.

use super::{
  Ctx, Element, FrameProps, Message, Press, RunControls, Services, ShownProject, StaleRead, TaskContext, TaskFrame, ci,
  control_action, header, merge, summary, team_controls,
};

impl TaskFrame {
  /// The latest run's merge attempts and the merge waiting in its worktree, as last read.
  pub(super) fn merge_reads(
    &self,
    detail: &orchester_workspace::tasks::detail::TaskDetail,
  ) -> (
    Option<std::sync::Arc<orchester_workspace::tasks::merge_attempts::page::AttemptPage>>,
    Option<orchester_workspace::tasks::resolve::Worktree>,
  ) {
    let run = detail.run.as_ref().map(|run| run.id());
    (
      run.and_then(|run| self.attempts.get().of(run)),
      run.and_then(|run| self.worktree.get().of(run)),
    )
  }

  /// The latest run's merge while it waits for review (ORC-731): the readers of its merge attempts and of the merge
  /// waiting in its worktree, which the top card, the header and ⌘↵ share, and what ⌘↵ presses now, if anything.
  pub(super) fn merge(&self, ctx: &mut Ctx, task: &TaskContext) -> (Vec<Element>, Option<Press>) {
    if summary::waiting(task) != Some(summary::Waiting::Ready) {
      return (Vec::new(), None);
    }
    let attempts = task.detail.run.as_ref().map(|run| {
      merge::reader::mount(
        ctx,
        &task.project,
        (&task.key, run.run.number(), run.id()),
        self.state.merge.again.get(),
        &self.attempts,
      )
    });
    let worktree = merge::worktree::mount(ctx, task, &self.worktree);
    let press = merge::Merge::of(task)
      .and_then(|standing| merge::action(ctx, task, standing))
      .and_then(|action| match action.control.offer {
        header::controls::Offer::Ready(press) => Some(press),
        _ => None,
      });
    (attempts.into_iter().chain(worktree).collect(), press)
  }
}

impl TaskFrame {
  /// Provides the run controls, the header's and the Summary's, and a team task's actions ([`team_controls`]).
  pub(super) fn provide_controls(&self, ctx: &mut Ctx, props: &FrameProps, services: &Services) {
    let runners = (services.runners.clone(), services.servers.clone());
    let ready = (services.workspace.clone(), services.vault.clone());
    let action = control_action(ctx, (props.project.open.clone(), props.task), runners, ready);
    ctx.provide(RunControls {
      action,
      requested: self.requested.clone(),
    });
    team_controls::provide(ctx, (&props.project, props.team.clone()), &self.taking_over);
  }
  /// Why the task could not be read again: `failed` now, else what failed before while the task is
  /// read again (`pending`), so the notice does not flicker; nothing once a reading succeeded.
  pub(super) fn stale(&self, failed: Option<Message>, pending: bool) -> Option<Message> {
    let kept = match failed {
      Some(reason) => Some(reason),
      None if pending => self.stale.get_untracked().0,
      None => None,
    };
    if self.stale.get_untracked().0 != kept {
      self.stale.set(StaleRead(kept.clone()));
    }
    kept
  }

  /// The run's pull request's CI (ORC-335): its reader beside the task while the page shows `detail`'s run in
  /// `project`, with the project's `saves`, and what the page shows of it; `online`: the project's runner is.
  pub(super) fn ci(
    &self,
    ctx: &mut Ctx,
    (project, detail): (&ShownProject, &orchester_workspace::tasks::detail::TaskDetail),
    (saves, online): (u64, bool),
  ) -> (Option<Element>, Option<ci::ShownCi>) {
    ctx.provide(self.ci_actions.clone());
    let again = (self.ci_actions.again.get(), saves);
    let reader = ci::reader::mount(ctx, (project, detail), again, &self.ci);
    let shown = ci::shown(&self.ci.get(), detail, detail.lost_its_runner(online));
    (reader, shown)
  }
}
