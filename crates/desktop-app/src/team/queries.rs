//! The lurq queries of a team project's reads (`docs/architecture/team-authority-plan.md` §6): each calls
//! `orchester_workspace::team_reads` and is kept by its [`TeamSource`]'s key (server, session generation, project) and
//! its own arguments: the view's statuses, the search, the filters and the order. Retention follows how each is read:
//! task listings change most and weigh most, so they are fresh for 30 s and kept 2 min unobserved; counts are small and
//! kept 5 min; the caller's capabilities, the modules and the admins' names change rarely. A listing reads at most
//! [`PAGES`] pages of the largest size and says when there are more; nothing reads every task of a project at once.

use std::{collections::HashMap, sync::Arc};

use orchester_api::v2::{
  CallerRole, Capabilities, MAX_PAGE_LIMIT, ModuleListRequest, PageRequest, SearchText, Subject, TaskBoardCounts,
  TaskFilters, TaskListRequest, TaskSort, WorkspacePolicy,
};
use orchester_core::{
  id::{ModuleId, ResourceId, TagId, TaskId},
  module::Module,
  sharing::ClaimStalenessThreshold,
  task::{
    Task,
    organization::{Priority, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_workspace::{
  background,
  sharing::project::{ProjectServer, TeamAdmins, convert},
  team_reads::{
    TeamRead,
    listed::{ListedTasks, TeamPipelines},
  },
};

use super::{
  lifecycle::Offered,
  pages,
  source::{TeamFailure, TeamSource},
};

/// The most pages of [`MAX_PAGE_LIMIT`] tasks one listing reads.
pub const PAGES: usize = 10;

/// What the Tasks list asks the server for: the statuses of its view, its search and filters, and its order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TaskListKey {
  pub statuses: Vec<WorkflowStatus>,
  pub search: Option<SearchText>,
  pub priorities: Vec<Priority>,
  pub tags: Vec<TagId>,
  pub modules: Vec<Option<ModuleId>>,
  /// The team's pipelines, sent by their resource ids.
  pub pipelines: Vec<Option<ResourceId>>,
  pub sort: TaskSort,
}

impl TaskListKey {
  /// The same search and filters over every status, as the views count them.
  pub fn counted(&self) -> Self {
    Self {
      statuses: Vec::new(),
      sort: TaskSort::default(),
      ..self.clone()
    }
  }

  /// The server's filters; a team task names its pipeline by its resource id.
  fn filters(&self) -> TaskFilters {
    TaskFilters {
      statuses: self.statuses.clone(),
      priorities: self.priorities.clone(),
      tags: self.tags.clone(),
      all_tags: false,
      pipelines: self
        .pipelines
        .iter()
        .map(|pipeline| pipeline.map(|id| id.to_string()))
        .collect(),
      modules: self.modules.clone(),
      releases: Vec::new(),
      search: self.search.clone(),
    }
  }
}

/// The tasks of one listing as the lists show them (`ListedTasks`), each part shared, so every render of one reading
/// is the same reading; `more` when the listing has tasks past the [`PAGES`] read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTasks {
  pub tasks: Arc<Vec<Task>>,
  pub updated: Arc<HashMap<TaskId, Timestamp>>,
  pub former_keys: Arc<HashMap<TaskId, String>>,
  pub missing_pipelines: Arc<HashMap<TaskId, String>>,
  pub blocking: Arc<HashMap<TaskId, Vec<TaskId>>>,
  /// What each task offers the person of its lifecycle, as its capabilities say.
  pub offered: Arc<HashMap<TaskId, Offered>>,
  pub more: bool,
}

impl TeamTasks {
  fn of(listed: ListedTasks, offered: HashMap<TaskId, Offered>, more: bool) -> Self {
    Self {
      tasks: Arc::new(listed.tasks),
      updated: Arc::new(listed.updated),
      former_keys: Arc::new(listed.former_keys),
      missing_pipelines: Arc::new(listed.missing_pipelines),
      blocking: Arc::new(listed.blocking),
      offered: Arc::new(offered),
      more,
    }
  }
}

#[lurq::query(stale_time = "30s", gc_time = "2m")]
pub async fn team_tasks(source: TeamSource, list: TaskListKey) -> Result<TeamTasks, TeamFailure> {
  let pipelines = team_pipelines(&source).await?;
  let filters = list.filters();
  let mut reads = Vec::new();
  let mut after = None;
  for _ in 0..PAGES {
    let page = PageRequest::new(after.take(), Some(MAX_PAGE_LIMIT)).expect("the largest page is a page");
    let request = TaskListRequest {
      filters: filters.clone(),
      sort: list.sort,
      page,
    };
    let page = source.answered(source.project().task_page(request).await)?.into_value();
    reads.extend(page.items);
    match page.next {
      Some(next) => after = Some(next),
      None => return listed(&reads, &pipelines, false),
    }
  }
  listed(&reads, &pipelines, true)
}

#[lurq::query(stale_time = "30s", gc_time = "5m")]
pub async fn team_counts(source: TeamSource, list: TaskListKey) -> Result<TaskBoardCounts, TeamFailure> {
  let read = source.project().board_counts(list.filters()).await;
  source.answered(read).map(TeamRead::into_value)
}

/// What the person may do in the workspace, as the server says it.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_capabilities(source: TeamSource) -> Result<Capabilities, TeamFailure> {
  let read = source.project().capabilities().await;
  source.answered(read).map(TeamRead::into_value)
}

/// The workspace's policy, and whether the person changes it: an Admin or a server admin, as the server's capabilities
/// say, never a role kept on this computer; and the server's claim staleness threshold, which only a server admin
/// changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamPolicy {
  pub policy: WorkspacePolicy,
  pub changes: bool,
  pub stale_after: ClaimStalenessThreshold,
}

#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_policy(source: TeamSource) -> Result<TeamPolicy, TeamFailure> {
  let read = source.project().metadata().await;
  let metadata = source.answered(read)?.into_value();
  Ok(TeamPolicy {
    policy: metadata.policy,
    changes: metadata.capabilities.role != CallerRole::Member,
    stale_after: metadata.claim_staleness_seconds,
  })
}

/// The workspace's modules, oldest first, for the filters and the board's columns.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_modules(source: TeamSource) -> Result<Vec<Module>, TeamFailure> {
  let request = ModuleListRequest {
    state: None,
    page: PageRequest::new(None, Some(MAX_PAGE_LIMIT)).expect("the largest page is a page"),
  };
  let page = source
    .answered(source.project().module_page(request).await)?
    .into_value();
  page
    .items
    .iter()
    .map(|summary| convert::cached_module(&summary.module.module).map(|cached| cached.module))
    .collect::<Result<_, _>>()
    .map_err(|error| {
      tracing::warn!(%error, "a team module cannot be shown");
      TeamFailure::Refused
    })
}

/// Who changes the workspace's tasks, modules and tags, as the members read names them.
#[lurq::query(stale_time = "5m", gc_time = "10m")]
pub async fn team_admins(source: TeamSource) -> Result<TeamAdmins, TeamFailure> {
  let (servers, root) = (source.servers().clone(), source.root().to_owned());
  let opened = background::run(move || ProjectServer::open(&servers, &root))
    .await
    .map_err(|_| TeamFailure::Refused)?;
  match opened {
    Ok(Some(server)) => server.team_admins().await.map_err(|error| TeamFailure::from(&error)),
    Ok(None) => Err(TeamFailure::Refused),
    Err(error) => Err(TeamFailure::from(&error)),
  }
}

/// The team's pipelines this computer has.
pub(super) async fn team_pipelines(source: &TeamSource) -> Result<TeamPipelines, TeamFailure> {
  let root = source.root().to_owned();
  match background::run(move || TeamPipelines::read(&root)).await {
    Ok(Ok(pipelines)) => Ok(pipelines),
    Ok(Err(error)) => {
      tracing::warn!(%error, "the project's pipelines cannot be read for its team tasks");
      Err(TeamFailure::Refused)
    }
    Err(_) => Err(TeamFailure::Refused),
  }
}

fn listed(
  reads: &[orchester_api::v2::TaskRead],
  pipelines: &TeamPipelines,
  more: bool,
) -> Result<TeamTasks, TeamFailure> {
  let offered = reads
    .iter()
    .map(|read| {
      let offered = Offered {
        abilities: pages::lifecycle(&read.capabilities, Subject::Task { id: read.task.id }),
        revision: read.task.revision,
        holder: read
          .effective_claim
          .as_ref()
          .map(|claim| claim.holder.display_name.clone()),
      };
      (read.task.id, offered)
    })
    .collect();
  ListedTasks::of(reads, |pipeline| pipelines.id(pipeline))
    .map(|listed| TeamTasks::of(listed, offered, more))
    .map_err(|error| {
      tracing::warn!(%error, "a team task cannot be shown");
      TeamFailure::Refused
    })
}
