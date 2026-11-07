//! Reads the latest run's merge attempts (`merge_attempts::page`, as its attempt page does) while it waits for review,
//! again whenever the project's tasks or runs change, the merge queue moves, a merge job starts or ends, or the page's
//! own work on the merge ended (`again`). What it read goes into the page's signal, set only when it changed, so the
//! top card, the header and ⌘↵ share one read. Renders nothing.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{id::RunId, project::TaskKey, run::RunNumber};
use orchester_workspace::tasks::merge_attempts::page::AttemptPage;

use crate::{
  project::ShownProject,
  screens::task::merge::attempt::read::{Read, read},
};

/// What was read, for run `0`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attempts(pub Option<(RunId, Arc<AttemptPage>)>);

impl DevtoolsInspectable for Attempts {}

impl Attempts {
  /// The attempts of `run`, once they were read.
  pub fn of(&self, run: RunId) -> Option<Arc<AttemptPage>> {
    self
      .0
      .as_ref()
      .filter(|(read, _)| *read == run)
      .map(|(_, page)| page.clone())
  }
}

/// The reader of run `number` (`run`) of the task `key` in `project`, putting what it reads into `into`; `again`
/// reads it anew.
pub fn mount(
  ctx: &mut Ctx,
  project: &ShownProject,
  (key, number, run): (&TaskKey, RunNumber, RunId),
  again: u64,
  into: &Signal<Attempts>,
) -> Element {
  let props = ReaderProps {
    project: project.clone(),
    key: key.clone(),
    number,
    run,
    again,
    into: into.clone(),
  };
  ctx.mount_keyed::<AttemptsReader>("task-merge-reader", props)
}

#[derive(Clone)]
pub struct ReaderProps {
  project: ShownProject,
  key: TaskKey,
  number: RunNumber,
  run: RunId,
  again: u64,
  into: Signal<Attempts>,
}

/// The page's own signal is the same for its life: it is compared by its id.
impl PartialEq for ReaderProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.key == other.key
      && self.number == other.number
      && self.run == other.run
      && self.again == other.again
      && self.into.id() == other.into.id()
  }
}

impl DevtoolsInspectable for ReaderProps {}

pub struct AttemptsReader;

impl Component for AttemptsReader {
  type Props = ReaderProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReaderProps>().clone();
    let read = match read(ctx, &props.project, (&props.key, props.number), props.again) {
      Read::Ready(page) => Some(Attempts(Some((props.run, page)))),
      Read::Missing => Some(Attempts(None)),
      // The card stays the Ready card, and the run's attempt page says why.
      Read::Failed(message) => {
        tracing::warn!(?message, run = %props.run, "cannot read the run's merge attempts for its task page");
        Some(Attempts(None))
      }
      Read::Reading => None,
    };
    if let Some(read) = read
      && props.into.get_untracked() != read
    {
      props.into.set(read);
    }
    Column::new().id("task-merge-reader")
  }
}
