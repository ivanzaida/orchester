//! What the Memory filter reads, off the UI thread: the project's memory whenever the project's tasks are saved or a
//! run changes state, and when the person tries again; and how many active records it has, which the hub's Memory
//! filter counts on every filter of the hub.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::{
  background,
  memory::read::{ProjectMemory, project_active_count_in},
  messages::{self, Message},
};

use crate::{
  project::{ShownProject, settled_read::settled_read},
  screens::{hub::agents::TabCount, memory::state::Shared},
  services::Services,
  ui::failure::LastFailure,
};

/// What the filter shows of the project's memory.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Read {
  Reading,
  Ready(Arc<ProjectMemory>),
  /// Why it couldn't be read, and whether it is being read again.
  Failed(Message, bool),
}

/// The filter shows no transcript, so a transcript's lines do not read it again: a record an agent saves while its run
/// runs shows once a run changes state (a step ends, a question or an approval waits).
pub(super) fn memory(ctx: &mut Ctx, project: &ShownProject, (retries, failure): (u64, &LastFailure)) -> Read {
  let feed = &project.feed;
  let deps = (
    project.open.repo_root.clone(),
    (feed.revision.get(), feed.states.get(), retries),
  );
  let hold = Services::of(ctx).hold;
  let loaded = settled_read(ctx, "memory", deps, move |(repo, _)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || ProjectMemory::load_in(&repo).map(Shared::new))
        .await?
        .map_err(|error| messages::store_error(&error))
    }
  });
  // Data kept from before a failed reread is not shown as current, nor while it is read again.
  if let Some((error, retrying)) = failure.follow(&loaded) {
    return Read::Failed(error, retrying);
  }
  loaded.data.map_or(Read::Reading, |read| Read::Ready(read.0))
}

/// How many active records the project's own memory has, read when its tasks are saved or a run changes state: its
/// bone while it is read the first time, no count when it cannot be read (the filter's own read says why once it is
/// opened, with *Try again*).
pub(in crate::screens) fn active_count(ctx: &mut Ctx, project: &ShownProject) -> TabCount {
  let feed = &project.feed;
  let deps = (project.open.repo_root.clone(), (feed.revision.get(), feed.states.get()));
  let hold = Services::of(ctx).hold;
  let loaded = settled_read(ctx, "memory count", deps, move |(repo, _)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || project_active_count_in(&repo))
        .await?
        .map_err(|error| messages::store_error(&error))
    }
  });
  if loaded.is_rejected() {
    return TabCount::Unknown;
  }
  TabCount::of(loaded.data)
}
