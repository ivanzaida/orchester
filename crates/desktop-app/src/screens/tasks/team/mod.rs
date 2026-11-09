//! A team project's Tasks list and board read from its server (`Team authority · redesign` `BZc66`, rows 1 and 6;
//! `docs/design/team-authority.md`): the view's tasks, its counts, the workspace's modules and what the person may do,
//! through the session's lurq queries (`crate::team`), joined with this computer's runs by task id. A member's list and
//! board change nothing: no New task, no quick add, no drag, and one calm line names who does change them
//! ([`lines`]). The person's standing is the server's (its capabilities' role), never a role kept here. While the
//! server does not answer, the list says team data is unavailable and shows only what runs on this computer
//! ([`outage`]); what was read before is never shown as current.
//!
//! The view asks the server for the workflow statuses it may hold, with the search, the filters and the order; the list
//! then keeps, groups and orders the rows as for a local project. The views are counted from the server's counts by
//! status, with the tasks something waits for on this computer placed as a local list places them
//! (`orchester_workspace::tasks::views`). Tag names are still this computer's: the server has no read for them yet.

pub(super) mod actions;
mod lines;
pub(super) mod menu;
pub mod outage;

use std::{collections::HashMap, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_api::v2::{CallerRole, SearchText, TaskSort as ServerSort};
use orchester_core::{id::TaskId, module::Module};
use orchester_workspace::{
  sharing::{Connection, project::TeamAdmins},
  tasks::{
    browse::TaskSort,
    data::ProjectTasks,
    views::{ListView, StatusCounts},
  },
};

pub(super) use self::lines::{lock_line, team_line};
use super::{cache::ViewCounts, header::Choices, shared::Shared, views::list_view};
use crate::{
  project::ShownProject,
  routes::TaskList,
  team::{
    TeamFailure, TeamSource,
    lifecycle::Offered,
    queries::{TaskListKey, TeamTasks, team_admins, team_capabilities, team_counts, team_modules, team_tasks},
    view::source,
  },
};

/// What a team project's list shows.
pub(super) enum Team {
  /// The project's source, the person's standing or the view's tasks are being read.
  Reading,
  Ready(Box<TeamShown>),
  /// Why the team's data cannot be shown.
  Failed(TeamFailure),
}

/// A team project's list as read: its tasks joined with this computer's runs, how many tasks each view holds, and
/// what the person may do there.
pub(super) struct TeamShown {
  pub tasks: ProjectTasks,
  pub counts: ViewCounts,
  pub role: CallerRole,
  /// Who changes the team's tasks, as the members read named them; `None` until read, or for an Admin.
  pub admins: Option<Arc<TeamAdmins>>,
  /// What each task offers the person of its lifecycle, as its capabilities say.
  pub offered: Arc<HashMap<TaskId, Offered>>,
  /// Where the lifecycle changes go.
  pub source: TeamSource,
  /// The view has more tasks than one reading takes.
  pub more: bool,
}

impl TeamShown {
  /// A member changes nothing: only Admins and Server admins do.
  pub fn member(&self) -> bool {
    self.role == CallerRole::Member
  }
}

/// The team list of `project` on `list`'s view, as `choices` narrow and order it; `shared` is how the project stands
/// with its server and `local` what this computer knows of its runs. The session's query client is provided before.
pub(super) fn read(
  ctx: &mut Ctx,
  (project, shared): (&ShownProject, Option<&Shared>),
  (list, choices): (&TaskList, &Choices),
  local: &ProjectTasks,
) -> Team {
  let source = match source(ctx, project) {
    Ok(Some(source)) => source,
    Ok(None) => return Team::Reading,
    Err(failure) => return Team::Failed(failure),
  };
  // While the connection monitor finds the server offline, what was read before is no longer current, whether or not
  // a read failed yet.
  if let Some(Connection::Offline { since, .. }) = shared.and_then(|shared| shared.sharing.connection) {
    return Team::Failed(TeamFailure::Unavailable { since });
  }
  let key = key(list, choices);
  let capabilities = ctx.query(team_capabilities(source.clone()));
  let tasks = ctx.query(team_tasks(source.clone(), key.clone()));
  let counts = ctx.query(team_counts(source.clone(), key.counted()));
  let modules = ctx.query(team_modules(source.clone()));
  let failed = [capabilities.error(), tasks.error(), counts.error(), modules.error()];
  if let Some(failure) = failed.into_iter().flatten().next() {
    return Team::Failed((*failure).clone());
  }
  let waiting = match waiting(ctx, &source, &key, local) {
    Ok(waiting) => waiting,
    Err(failure) => return Team::Failed(failure),
  };
  let (Some(capabilities), Some(read), Some(counted), Some(modules), Some(waiting)) = (
    capabilities.data(),
    tasks.data(),
    counts.data(),
    modules.data(),
    waiting,
  ) else {
    return Team::Reading;
  };
  let admins = (capabilities.role == CallerRole::Member)
    .then(|| ctx.query(team_admins(source.clone())).data())
    .flatten();
  Team::Ready(Box::new(TeamShown {
    tasks: joined(local, &read, modules),
    counts: ViewCounts::of_server(&StatusCounts::of_board(&counted), &waiting),
    role: capabilities.role,
    admins,
    offered: read.offered.clone(),
    source,
    more: read.more,
  }))
}

/// How many of the tasks something waits for the person in on this computer are in each open status, under the view's
/// search and filters: read from the server's open tasks, the listing Needs you and Active read too, only while
/// something waits. `None` while they are read.
fn waiting(
  ctx: &mut Ctx,
  source: &TeamSource,
  key: &TaskListKey,
  local: &ProjectTasks,
) -> Result<Option<StatusCounts>, TeamFailure> {
  let waiting = local.waiting_tasks();
  if waiting.is_empty() {
    return Ok(Some(StatusCounts::default()));
  }
  let open = TaskListKey {
    statuses: ListView::NeedsYou.statuses(),
    ..key.clone()
  };
  let open = ctx.query(team_tasks(source.clone(), open));
  if let Some(failure) = open.error() {
    return Err((*failure).clone());
  }
  Ok(open.data().map(|read| {
    StatusCounts::of(
      read
        .tasks
        .iter()
        .filter(|task| waiting.contains(&task.id()))
        .map(|task| task.organization().status()),
    )
  }))
}

/// This computer's knowledge of the project with the server's tasks and modules in place of its own: its runs, what
/// waits for the person and its pipelines stay this computer's.
fn joined(local: &ProjectTasks, read: &TeamTasks, modules: Arc<Vec<Module>>) -> ProjectTasks {
  ProjectTasks {
    tasks: read.tasks.clone(),
    updated: read.updated.clone(),
    former_keys: read.former_keys.clone(),
    missing_pipelines: read.missing_pipelines.clone(),
    blocking: read.blocking.clone(),
    modules,
    not_shared: Arc::default(),
    kept_tags: Arc::default(),
    shared: true,
    ..local.clone()
  }
}

/// What the list asks the server for: the workflow statuses its view may hold (`ListView::statuses`, which the rows
/// are then kept by), its search and filters, and its order. A search the server would refuse (over 200 characters) is
/// not sent; the list still searches what comes back.
fn key(list: &TaskList, choices: &Choices) -> TaskListKey {
  let facets = choices.facets.get();
  TaskListKey {
    statuses: list_view(list.tab()).statuses(),
    search: SearchText::new(list.search().unwrap_or_default()).ok(),
    priorities: facets.priorities,
    tags: facets.tags,
    modules: facets.modules,
    pipelines: facets.pipelines,
    sort: match choices.sort.get() {
      TaskSort::Manual => ServerSort::Created,
      TaskSort::Newest => ServerSort::Newest,
      TaskSort::Title => ServerSort::Title,
      TaskSort::Status => ServerSort::Status,
      TaskSort::Priority => ServerSort::Priority,
    },
  }
}
