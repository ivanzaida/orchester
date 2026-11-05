//! Reads the latest run's worktree and the merge waiting in it for the person to resolve it (`resolve::read`) while
//! the run waits for review, again when the project's tasks are saved, the run's resolving starts or ends, and every
//! [`LOOK`] while a merge waits: the person resolves it in an editor, which tells Orchester nothing. What it read goes
//! into the page's signal, set only when it changed, so the top card, the header and ⌘↵ share one read: the action
//! that merges waits while a merge waits there. Renders nothing.

use std::{path::PathBuf, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  tasks::resolve::{self, Worktree},
};

use crate::{screens::task::TaskContext, services::Services};

/// How often a merge waiting in the run's worktree is read again while the person resolves it.
const LOOK: Duration = Duration::from_secs(2);

/// What was read, for run `0`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InWorktree(pub Option<(RunId, Worktree)>);

impl DevtoolsInspectable for InWorktree {}

impl InWorktree {
  /// The worktree of `run`, once it was read.
  pub fn of(&self, run: RunId) -> Option<Worktree> {
    self
      .0
      .as_ref()
      .filter(|(read, _)| *read == run)
      .map(|(_, worktree)| worktree.clone())
  }
}

/// The reader of `task`'s latest run's worktree, putting what it reads into `into`; `None` without a run.
pub fn mount(ctx: &mut Ctx, task: &TaskContext, into: &Signal<InWorktree>) -> Option<Element> {
  let run = task.detail.run.as_ref()?.id();
  let job = task.jobs.run(run);
  let props = ReaderProps {
    root: task.project.open.repo_root.clone(),
    run,
    idle: !job.resolving(),
    again: (task.project.feed.revision.get(), job.resolve.ended),
    into: into.clone(),
  };
  Some(ctx.mount_keyed::<WorktreeReader>("task-worktree-reader", props))
}

#[derive(Clone)]
pub struct ReaderProps {
  root: PathBuf,
  run: RunId,
  /// No resolving of the run is under way.
  idle: bool,
  /// The project's saves and the run's resolvings ended: either reads it anew.
  again: (u64, u64),
  into: Signal<InWorktree>,
}

/// The page's own signal is the same for its life: it is compared by its id.
impl PartialEq for ReaderProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.run == other.run
      && self.idle == other.idle
      && self.again == other.again
      && self.into.id() == other.into.id()
  }
}

impl DevtoolsInspectable for ReaderProps {}

pub struct WorktreeReader {
  /// Counts while a merge waits in the run's worktree, which is read again then.
  look: Signal<u64>,
  /// Ticks [`Self::look`]; it runs only while a merge waits in the worktree.
  looking: Interval,
}

impl Component for WorktreeReader {
  type Props = ReaderProps;

  fn create(ctx: &mut Ctx) -> Self {
    let look = ctx.signal(0_u64);
    let next = look.clone();
    let looking = ctx.create_interval(LOOK, move || next.update(|tick| *tick += 1));
    Self { look, looking }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReaderProps>().clone();
    let read = read(
      ctx,
      props.root.clone(),
      (props.run, props.idle),
      (props.again, self.look.get()),
    );
    if read.as_ref().is_some_and(|worktree| worktree.merge.is_some()) {
      self.looking.start();
    } else {
      self.looking.stop();
    }
    if let Some(worktree) = read {
      let read = InWorktree(Some((props.run, worktree)));
      if props.into.get_untracked() != read {
        props.into.set(read);
      }
    }
    Column::new().id("task-worktree-reader")
  }
}

/// What [`read`] read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Worktree);

impl DevtoolsInspectable for Read {}

/// `run`'s worktree under `root` and the merge waiting in it, read again when `again` (the project's saves and the
/// run's resolvings ended, and a look) changes; `idle`: no resolving of the run is under way. `None` while it is first
/// read, or when it cannot be read.
pub fn read(ctx: &mut Ctx, root: PathBuf, (run, idle): (RunId, bool), again: ((u64, u64), u64)) -> Option<Worktree> {
  let programs = Services::of(ctx).programs;
  ctx
    .future((root, (run, idle), again), move |(root, (run, idle), _)| {
      let programs = programs.clone();
      async move {
        background::run(move || resolve::read(&programs, &root, (run, idle)))
          .await
          .map_err(|_| ())?
          .map(Read)
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
