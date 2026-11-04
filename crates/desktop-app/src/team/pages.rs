//! The lurq queries of a team project's pages (`docs/architecture/team-authority-plan.md` §6, ORC-717): one task by its
//! key, the workspace's modules and one module, each with its assignee and the claim its work runs under, read from
//! `orchester_workspace::team_reads` and kept, as the lists' queries are ([`super::queries`]), by the [`TeamSource`]'s
//! key and their own arguments. A claim or an assignee says whether it is the person signed in to the server; who that
//! is, is read from this computer's servers, never sent anywhere.

use orchester_api::{
  UserRef,
  v2::{
    ClaimRead, MAX_PAGE_LIMIT, ModuleListRequest, ModuleSummary, PageRequest, SearchText, TaskFilters, TaskListRequest,
    TaskRead,
  },
};
use orchester_core::{
  forecast::Forecast,
  id::{ModuleId, UserId},
  module::{Module, progress::Progress},
  project::TaskKey,
  task::Task,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  sharing::project::convert,
  team_reads::{TeamRead, TeamReadError, listed},
};

use super::{
  queries::slugs,
  source::{TeamFailure, TeamSource},
};

/// Someone a team read names: their name on the server, and whether they are the person signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPerson {
  pub name: String,
  pub you: bool,
}

/// The claim a task's or a module's work runs under, as the server reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamClaim {
  pub holder: TeamPerson,
  /// When the holder's work last renewed it.
  pub renewed: Timestamp,
  /// When it goes stale without another renewal, by the server's threshold.
  pub stale_at: Timestamp,
  pub stale: bool,
  /// A task's claim taken under its module's.
  pub under_module: bool,
}

/// A team task as its page shows it: the task, the slug of a pipeline this computer has none for, its assignee and its
/// claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTask {
  pub task: Task,
  pub missing_pipeline: Option<String>,
  pub assignee: Option<TeamPerson>,
  pub claim: Option<TeamClaim>,
}

/// A team module as its row and its page show it: the module, its progress and forecast as the server counts them, how
/// many tasks it has (Canceled ones too), its branch, its assignee and its claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamModule {
  pub module: Module,
  pub progress: Progress,
  pub tasks: u32,
  /// `None` when the server's forecast names a day that is no day.
  pub forecast: Option<Forecast>,
  pub branch: String,
  pub assignee: Option<TeamPerson>,
  pub claim: Option<TeamClaim>,
}

/// The task `key`, found by the server's search for it; `None` when the project has no such task.
#[lurq::query(stale_time = "30s", gc_time = "5m")]
pub async fn team_task(source: TeamSource, key: TaskKey) -> Result<Option<TeamTask>, TeamFailure> {
  let shown = key.to_string();
  let request = TaskListRequest {
    filters: TaskFilters {
      search: SearchText::new(&shown).ok(),
      ..TaskFilters::default()
    },
    sort: Default::default(),
    page: PageRequest::new(None, Some(MAX_PAGE_LIMIT)).expect("the largest page is a page"),
  };
  let page = source.answered(source.project().task_page(request).await)?.into_value();
  let Some(read) = page.items.into_iter().find(|read| read.task.key == shown) else {
    return Ok(None);
  };
  let (slugs, you) = (slugs(&source).await?, you(&source).await);
  team_task_of(&read, &slugs, you).map(Some)
}

/// The workspace's modules, oldest first.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_module_list(source: TeamSource) -> Result<Vec<TeamModule>, TeamFailure> {
  let request = ModuleListRequest {
    state: None,
    page: PageRequest::new(None, Some(MAX_PAGE_LIMIT)).expect("the largest page is a page"),
  };
  let page = source
    .answered(source.project().module_page(request).await)?
    .into_value();
  let you = you(&source).await;
  page.items.iter().map(|summary| team_module_of(summary, you)).collect()
}

/// The module `module`; `None` when the workspace has no such module.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_module(source: TeamSource, module: ModuleId) -> Result<Option<TeamModule>, TeamFailure> {
  let summary = match source.project().module(module).await {
    Err(TeamReadError::NotFound) => return Ok(None),
    read => source.answered(read).map(TeamRead::into_value)?,
  };
  team_module_of(&summary, you(&source).await).map(Some)
}

/// Who is signed in to the source's server on this computer; `None` when that cannot be read.
async fn you(source: &TeamSource) -> Option<UserId> {
  let (servers, server) = (source.servers().clone(), source.key().server);
  let entries = background::run(move || servers.entries()).await.ok()?.ok()?;
  let entry = entries.into_iter().find(|entry| entry.stored.id == server)?;
  entry.stored.account.map(|account| account.user)
}

fn person(user: &UserRef, you: Option<UserId>) -> TeamPerson {
  TeamPerson {
    name: user.display_name.clone(),
    you: you == Some(user.id),
  }
}

fn claim(read: &ClaimRead, you: Option<UserId>) -> TeamClaim {
  TeamClaim {
    holder: person(&read.holder, you),
    renewed: Timestamp::from_unix_millis(read.last_execution_heartbeat_at),
    stale_at: Timestamp::from_unix_millis(read.stale_at),
    stale: read.stale,
    under_module: read.parent_module_claim_id.is_some(),
  }
}

fn team_task_of(read: &TaskRead, slugs: &listed::PipelineSlugs, you: Option<UserId>) -> Result<TeamTask, TeamFailure> {
  let slug = read.task.content.pipeline.as_deref();
  let pipeline = slug.and_then(|slug| slugs.id(slug));
  let task = listed::task_of(read, pipeline).map_err(|error| {
    tracing::warn!(%error, "a team task cannot be shown");
    TeamFailure::Refused
  })?;
  Ok(TeamTask {
    task,
    missing_pipeline: slug.filter(|_| pipeline.is_none()).map(str::to_owned),
    assignee: read.assignee.as_ref().map(|user| person(user, you)),
    claim: read.effective_claim.as_ref().map(|read| claim(read, you)),
  })
}

fn team_module_of(summary: &ModuleSummary, you: Option<UserId>) -> Result<TeamModule, TeamFailure> {
  let read = &summary.module;
  let module = convert::cached_module(&read.module).map_err(|error| {
    tracing::warn!(%error, "a team module cannot be shown");
    TeamFailure::Refused
  })?;
  Ok(TeamModule {
    module: module.module,
    progress: summary.progress.to_core(),
    tasks: summary.tasks,
    forecast: summary.forecast.to_core(),
    branch: read.branch.clone(),
    assignee: read.assignee.as_ref().map(|user| person(user, you)),
    claim: read.effective_claim.as_ref().map(|read| claim(read, you)),
  })
}
