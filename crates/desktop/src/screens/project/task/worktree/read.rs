//! What the task's runs without live work left in the repository: their worktrees and branches,
//! read with one worktree listing for all of them, for the topbar's *Remove worktree*, Ready for
//! review, Run again and *Runs of this task*.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{id::RunId, integration::Leftovers, run::Run};

use super::super::TaskProps;
use crate::{
  background,
  messages::Message,
  programs::Programs,
  tasks::{
    detail::TaskDetail,
    history::RunRecord,
    integration::{LeftBehind, leftovers_in},
  },
};

/// What the runs left, by run, or why it could not be read; `stale` while it is read again after a
/// merge or a removal.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::screens::project::task) struct TaskLeftovers {
  left: Result<Arc<[(RunId, Leftovers)]>, Message>,
  stale: bool,
}

impl lurq::app::component::DevtoolsInspectable for TaskLeftovers {}

impl TaskLeftovers {
  /// What `run` left, for its actions; `None` when it was not read (the run has live work) or
  /// is being read again after a merge or a removal, which must not show beside the new merge or
  /// offer a removal again.
  pub(in crate::screens::project::task) fn of(&self, run: RunId) -> Option<Result<Leftovers, Message>> {
    if self.stale {
      return None;
    }
    self.as_read(run)
  }

  /// What `run` left as last read, for its row, which keeps its lines while they are read again.
  pub(in crate::screens::project::task) fn as_read(&self, run: RunId) -> Option<Result<Leftovers, Message>> {
    match &self.left {
      Ok(left) => left
        .iter()
        .find(|(id, _)| *id == run)
        .map(|(_, leftovers)| Ok(leftovers.clone())),
      Err(error) => Some(Err(error.clone())),
    }
  }
}

/// What was read, with what the person had changed when it was read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
  read_at: (u64, bool),
  left: LeftBehind,
}

impl lurq::app::component::DevtoolsInspectable for Read {}

/// What the runs of `detail` without live work left, read again when one more run ends, when the
/// person merged or removed something, or when tasks were saved; `None` while there is no such run
/// or until it was read.
pub(in crate::screens::project::task) fn task_leftovers(
  ctx: &mut Ctx,
  props: &TaskProps,
  detail: &TaskDetail,
) -> Option<TaskLeftovers> {
  let runs: Vec<Arc<Run>> = detail
    .history
    .iter()
    .filter_map(RunRecord::without_live_work)
    .cloned()
    .collect();
  if runs.is_empty() {
    return None;
  }
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let separator = ctx.t("common.listSeparator");
  // A merge or a removal changes what is left: until it is read again, what was read before must
  // not show beside the new merge or reappear after the removal. Runner events change nothing of a
  // run without live work; a run that ends, or a new run (Run again removes worktrees first),
  // changes the runs read.
  let changed_by_person = (props.revision.0, detail.merge.is_some());
  let deps = (
    props.project.repo_root.clone(),
    runs,
    detail.history.len(),
    changed_by_person,
  );
  let load = ctx.future(deps, move |(repo_root, runs, _, read_at)| {
    let (programs, separator) = (programs.clone(), separator.clone());
    async move {
      background::run(move || leftovers_in(&repo_root, &programs, &runs))
        .await?
        .map(|left| Read { read_at, left })
        .map_err(|problem| problem.message(&separator))
    }
  });
  let state = load.state().get();
  let stale = state.is_pending()
    && state
      .data
      .as_ref()
      .is_some_and(|read| read.read_at != changed_by_person);
  match (state.data, state.error) {
    (_, Some(error)) => Some(TaskLeftovers {
      left: Err(error),
      stale,
    }),
    (
      Some(Read {
        left: LeftBehind(left), ..
      }),
      None,
    ) => Some(TaskLeftovers {
      left: Ok(left.into()),
      stale,
    }),
    (None, None) => None,
  }
}
