//! A team project's Tasks list and board read from its server (`Team authority · redesign` `BZc66`, rows 1 and 6;
//! `docs/design/team-authority.md`): the view's tasks, its counts, the workspace's modules and what the person may do,
//! through the session's lurq queries (`crate::team`), joined with this computer's runs by task id. A member's list and
//! board change nothing: no New task, no quick add, no drag, and one calm line names who does change them
//! ([`lines`]). The person's standing is the server's (its capabilities' role), never a role kept here. While the
//! server does not answer, the list says team data is unavailable and shows only what runs on this computer
//! ([`outage`]); what was read before is never shown as current.
//!
//! The view asks the server for its workflow statuses, with the search, the filters and the order; the list then
//! groups and orders the rows as for a local project. Tag names are still this computer's: the server has no read for
//! them yet.

mod lines;
pub mod outage;

use std::sync::Arc;

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_api::v2::{CallerRole, SearchText, TaskSort as ServerSort};
use orchester_core::{module::Module, task::organization::WorkflowStatus};
use orchester_workspace::{
  sharing::{Connection, project::TeamAdmins},
  tasks::{browse::TaskSort, data::ProjectTasks},
};

pub(super) use self::lines::{lock_line, team_line};
use super::{cache::ViewCounts, header::Choices, shared::Shared};
use crate::{
  project::ShownProject,
  routes::{TaskList, TaskListTab},
  screens::settings::orchester::follow,
  services::Services,
  team::{
    TeamFailure, TeamQueries, TeamSource,
    queries::{TaskListKey, TeamTasks, team_admins, team_capabilities, team_counts, team_modules, team_tasks},
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
  let (Some(capabilities), Some(read), Some(counted), Some(modules)) =
    (capabilities.data(), tasks.data(), counts.data(), modules.data())
  else {
    return Team::Reading;
  };
  let admins = (capabilities.role == CallerRole::Member)
    .then(|| ctx.query(team_admins(source.clone())).data())
    .flatten();
  let needs_you = local.waiting_tasks().len();
  Team::Ready(Box::new(TeamShown {
    tasks: joined(local, &read, modules),
    counts: ViewCounts::of_server(&counted, needs_you),
    role: capabilities.role,
    admins,
    more: read.more,
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

/// The project's source in the session's current generation, read again when the generation or a server changes;
/// `Ok(None)` until it is read.
fn source(ctx: &mut Ctx, project: &ShownProject) -> Result<Option<TeamSource>, TeamFailure> {
  let services = Services::of(ctx);
  let generation = generation(ctx, &services.team);
  let revision = follow(ctx, (&services.servers, &services.cloud));
  let root = project.open.repo_root.clone();
  let team = services.team.clone();
  let state = ctx
    .future((root, generation, revision), move |(root, ..)| {
      let team = team.clone();
      async move { team.source(&root).await }
    })
    .state()
    .get();
  let pending = state.is_pending();
  match (state.data, state.error) {
    (_, Some(failure)) if !pending => Err(failure),
    (Some(Some(source)), _) if source.key().generation == generation => Ok(Some(source)),
    _ => Ok(None),
  }
}

/// The session's team generation, read again when it ends.
fn generation(ctx: &mut Ctx, team: &TeamQueries) -> u64 {
  let stream = ctx.stream(team.clone(), |team, emitter: StreamEmitter<u64, ()>| async move {
    let mut generations = team.subscribe();
    loop {
      let generation = *generations.borrow_and_update();
      if !emitter.emit(generation) || generations.changed().await.is_err() {
        return;
      }
    }
  });
  stream.state().get().data.unwrap_or_else(|| team.generation())
}

/// What the list asks the server for: its view's workflow statuses, its search and filters, and its order. A search the
/// server would refuse (over 200 characters) is not sent; the list still searches what comes back.
fn key(list: &TaskList, choices: &Choices) -> TaskListKey {
  let facets = choices.facets.get();
  TaskListKey {
    statuses: statuses(list.tab()),
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

/// The workflow statuses a view holds, as `orchester_workspace::tasks::views` keeps them; every status for All, and
/// every open one for Needs you, whose tasks this computer's runs pick.
fn statuses(tab: TaskListTab) -> Vec<WorkflowStatus> {
  use WorkflowStatus::{Backlog, Canceled, Done, InProgress, InReview, Todo};
  match tab {
    TaskListTab::NeedsYou => vec![Backlog, Todo, InProgress, InReview],
    TaskListTab::Active => vec![Todo, InProgress, InReview],
    TaskListTab::Backlog => vec![Backlog],
    TaskListTab::Done => vec![Done, Canceled],
    TaskListTab::All => Vec::new(),
  }
}
