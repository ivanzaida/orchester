//! A team project's Tasks count beside its place in the sidebar: the server's Active count, from the count read the
//! list's views take with no search or filter (`crate::team::queries::team_counts`), never this computer's copy of
//! the tasks, with the tasks something waits for on this computer placed as the list's Active places them
//! (`crate::screens::tasks::team::waiting`), so the sidebar and the Active tab say the same number.

use lurq::app::ctx::Ctx;
use orchester_workspace::tasks::views::{ListView, StatusCounts};

use crate::{
  project::ShownProject,
  screens::tasks::team::waiting,
  team::{
    queries::{TaskListKey, team_counts},
    view::{provide_client, source},
  },
};

/// What the sidebar says of a team project's tasks.
pub(super) enum TeamCount {
  /// The server's count is read.
  Reading,
  Counted(usize),
  /// The team's data cannot be read now; the Tasks screen says why.
  Unread,
}

/// The Tasks count of the team project `shown`.
pub(super) fn team_count(ctx: &mut Ctx, shown: &ShownProject) -> TeamCount {
  provide_client(ctx);
  let source = match source(ctx, shown) {
    Ok(Some(source)) => source,
    Ok(None) => return TeamCount::Reading,
    Err(_) => return TeamCount::Unread,
  };
  let state = shown.feed.tasks.get();
  let Some(local) = state.ready() else {
    return TeamCount::Reading;
  };
  let key = TaskListKey::default();
  let counts = ctx.query(team_counts(source.clone(), key.clone()));
  if counts.error().is_some() {
    return TeamCount::Unread;
  }
  let waiting = match waiting(ctx, &source, &key, local) {
    Ok(Some(waiting)) => waiting,
    Ok(None) => return TeamCount::Reading,
    Err(_) => return TeamCount::Unread,
  };
  counts.data().map_or(TeamCount::Reading, |counts| {
    let counted = StatusCounts::of_board(&counts);
    TeamCount::Counted(ListView::Active.count_by_status(&counted, &waiting))
  })
}
