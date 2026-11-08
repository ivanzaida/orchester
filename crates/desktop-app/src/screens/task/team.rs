//! A team task's page (`01c · Task · member · claimed by you on this computer` `Tb55G`, ORC-717): the task as its
//! server reads it, found by its key through the session's lurq queries (`crate::team::pages`), with its assignee, its
//! claim, its module's name and branch and the person's standing, then shown in the same frame as a local task's,
//! joined by its id with what this computer knows of its runs (`TaskDetail::of_team`). A member changes nothing there;
//! the Summary names who does (`summary::team`). While the team's data cannot be read the page says so, as a team
//! module's does (`crate::team::outage`), and keeps nothing read before as current.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::project::TaskKey;

use crate::{
  project::ShownProject,
  team::{
    TeamFailure, TeamSource,
    last_start::team_last_start,
    outage,
    pages::{TeamModule, TeamTask, team_module_list, team_task},
    start::Recorded,
    view::{self, Standing},
  },
};

/// A team task as its page shows it, with the project's reads it was read from, which its actions go to, and the name
/// of the team's server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTaskShown {
  pub task: TeamTask,
  pub standing: Standing,
  /// The module the task is in, as the server reads it.
  pub module: Option<TeamModule>,
  pub source: TeamSource,
  pub server: Option<String>,
  /// The task's newest start as this computer's records tell it, once read: a start that waited and was settled later
  /// by recovery says why it did not run.
  pub last_start: Option<Recorded>,
}

impl lurq::app::component::DevtoolsInspectable for TeamTaskShown {}

/// What the page shows of a team task.
pub(super) enum TeamPage {
  Reading,
  Missing,
  Failed(TeamFailure),
  Ready(Arc<TeamTaskShown>),
}

/// The task `key` of the team project `project`, with the session's query client provided.
pub(super) fn read(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> TeamPage {
  view::provide_client(ctx);
  let server = view::place(ctx, project).map(|(_, name)| name);
  let source = match view::source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return TeamPage::Reading,
    Err(failure) => return TeamPage::Failed(failure),
  };
  let standing = match view::standing(ctx, &source) {
    Ok(Some(standing)) => standing,
    Ok(None) => return TeamPage::Reading,
    Err(failure) => return TeamPage::Failed(failure),
  };
  let read = ctx.query(team_task(source.clone(), key.clone()));
  let modules = ctx.query(team_module_list(source.clone()));
  if let Some(failure) = [read.error(), modules.error()].into_iter().flatten().next() {
    return TeamPage::Failed((*failure).clone());
  }
  let (Some(found), Some(modules)) = (read.data(), modules.data()) else {
    return TeamPage::Reading;
  };
  let Some(task) = (*found).clone() else {
    return TeamPage::Missing;
  };
  let module = task
    .task
    .module()
    .and_then(|id| modules.iter().find(|module| module.module.id() == id).cloned());
  // Not waited for: the page shows without it, and the notice joins it once read.
  let last_start = ctx
    .query(team_last_start(source.clone(), task.task.id()))
    .data()
    .and_then(|last| (*last).clone());
  TeamPage::Ready(Arc::new(TeamTaskShown {
    task,
    standing,
    module,
    source,
    server,
    last_start,
  }))
}

/// The page while the team's data cannot be read: why, and *Try again*.
pub(super) fn unavailable(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, failure: &TeamFailure) -> Element {
  let place = view::place(ctx, project);
  let unread = outage::Unread {
    id: "task-team-unavailable",
    title: ctx.t_args("task.team.unavailableTitle", [("task", key.to_string())]),
    unavailable: "task.team.unavailableBody",
  };
  let card = outage::card(ctx, &unread, (failure, &place, project.open.project.name().as_str()));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(card)
    .into()
}
