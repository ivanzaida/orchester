//! `list_pull_requests` and `get_pull_request`: the pull requests of a project's linked GitHub repository
//! as the Pull requests page lists them ([`crate::pull_requests::list`]), with their state, checks, review
//! state and the task and run whose branch they are of. They read what the last sync stored and how the
//! latest sync went; `refresh` syncs first through `gh`, as the page's Refresh does, unless a sync ran in the
//! last 30 s or a rate limit holds syncs back: then the stored list answers, saying so. Every item carries its
//! app `link` and its GitHub `url`.

use orchester_core::{pull_request::SyncFailure, repository::RepositoryError, time::Timestamp};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{across, needs_project},
  pull_request_reads::ReadOne,
  pull_request_wire::{item, repository},
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  pull_requests::{
    InboxError,
    list::{
      LinkedList, ListedPullRequest, PullRequestList, PullRequestSort, PullRequestTab, load_list_in, read_list,
      refresh_list_in,
    },
    page::{PullRequestPage, PullRequestView, refresh_page_in},
    status::SyncStatus,
  },
  workspace::OpenProject,
};

/// A tab of the Pull requests page.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Tab {
  /// Open, drafts included.
  Open,
  /// Open, ready for review, and GitHub asks for a review.
  NeedsReview,
  /// Open, and the run that made its branch is running.
  AgentWorking,
  Merged,
  /// Closed without merging.
  Closed,
}

impl From<Tab> for PullRequestTab {
  fn from(tab: Tab) -> Self {
    match tab {
      Tab::Open => Self::Open,
      Tab::NeedsReview => Self::NeedsReview,
      Tab::AgentWorking => Self::AgentWorking,
      Tab::Merged => Self::Merged,
      Tab::Closed => Self::Closed,
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListPullRequests {
  /// The project's id, or its name; without it, every project's, each pull request naming its project.
  #[serde(default)]
  pub project: Option<String>,
  /// Only the pull requests of this tab of the Pull requests page; without it, every one the last sync read.
  #[serde(default)]
  pub tab: Option<Tab>,
  /// A number (`42`, `#42`), words of the title, a branch, or a task's key.
  #[serde(default)]
  pub search: Option<String>,
  /// Syncs with GitHub through `gh` first, as the page's Refresh does; needs a project. A sync that ran in the
  /// last 30 s, or a rate limit, answers from what is stored instead.
  #[serde(default)]
  pub refresh: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetPullRequest {
  /// The project's id, or its name.
  pub project: String,
  /// The pull request's number.
  pub number: u64,
  /// Syncs with GitHub through `gh` first, as list_pull_requests' refresh does; a pull request the sync does
  /// not list (an older closed one) is then read on its own, at most once in 30 s.
  #[serde(default)]
  pub refresh: bool,
}

pub async fn list_pull_requests(
  services: &Services,
  args: ListPullRequests,
  translate: &Translate,
) -> Result<Value, ToolError> {
  if args.refresh && args.project.is_none() {
    return Err(needs_project("refresh"));
  }
  let services = services.clone();
  let (named, refresh) = (args.project.clone(), args.refresh);
  let read = blocking(move || {
    across(&services, named.as_deref(), |open, db| {
      if refresh {
        refreshed(&services, open)
      } else {
        read_list(db).map_err(|error| refused(&error))
      }
    })
  })
  .await?;
  let query = args.search.as_deref().unwrap_or_default();
  let pull_requests = read.items(|open, list| {
    let PullRequestList::Linked(linked) = list else {
      return Vec::new();
    };
    let rows = match args.tab {
      Some(tab) => linked.rows(tab.into(), query, PullRequestSort::RecentlyUpdated),
      None => all_rows(linked, query),
    };
    rows
      .iter()
      .map(|listed| item(open, open.project.id(), listed))
      .collect()
  });
  let repositories = read.items(|open, list| vec![repository(open, list)]);
  Ok(json!({
    "pull_requests": pull_requests,
    "repositories": repositories,
    "unreadable_projects": read.unreadable(translate),
  }))
}

pub async fn get_pull_request(services: &Services, args: GetPullRequest) -> Result<Value, ToolError> {
  let services = services.clone();
  let number = args.number;
  let (project, list, listed) = blocking(move || {
    let project = services.project(&args.project)?;
    let list = if args.refresh {
      refreshed(&services, &project)?
    } else {
      load_list_in(&project.repo_root).map_err(|error| refused(&error))?
    };
    let listed = match &list {
      PullRequestList::Unlinked => return Err(ToolError::not_found(Missing::PullRequest, format!("#{number}"))),
      PullRequestList::Linked(linked) => match linked.find(number) {
        Some(listed) => listed.clone(),
        None if !args.refresh => return Err(not_synced(number)),
        None => match &linked.failure {
          Some(failure) if failure.is_paused(Timestamp::now()) => return Err(not_read(number, failure)),
          _ => read_one(&services, &project, number)?,
        },
      },
    };
    Ok((project, list, listed))
  })
  .await?;
  let mut found = item(&project, project.project.id(), &listed);
  found["repository"] = repository(&project, &list);
  Ok(found)
}

/// The list of `project` after a client's refresh.
fn refreshed(services: &Services, project: &OpenProject) -> Result<PullRequestList, ToolError> {
  refresh_list_in(&project.repo_root, &services.programs, Timestamp::now()).map_err(|error| refused(&error))
}

fn refused(error: &InboxError) -> ToolError {
  ToolError::refused("project_data", vec![error.message()])
}

/// Pull request `number`, which the sync did not list, read on its own from GitHub, unless it was less than
/// 30 s ago: then what that read came to answers.
fn read_one(services: &Services, project: &OpenProject, number: u64) -> Result<ListedPullRequest, ToolError> {
  let key = (project.project.id(), number);
  let now = Timestamp::now();
  let read = match services.pull_request_reads.recent(key, now) {
    Some(read) => read,
    None => {
      let read = read_from_github(services, project, number)?;
      services.pull_request_reads.keep(key, now, read.clone());
      read
    }
  };
  match read {
    ReadOne::Read(listed) => Ok(*listed),
    ReadOne::NotFound => Err(ToolError::not_found(Missing::PullRequest, format!("#{number}"))),
    ReadOne::Failed(failure) => Err(not_read(number, &failure)),
  }
}

/// What asking GitHub for pull request `number` came to.
fn read_from_github(services: &Services, project: &OpenProject, number: u64) -> Result<ReadOne, ToolError> {
  let page = refresh_page_in(&project.repo_root, number, &services.programs).map_err(|error| refused(&error))?;
  let PullRequestPage::Linked(view) = page else {
    return Ok(ReadOne::NotFound);
  };
  let PullRequestView {
    pull_request,
    task,
    failure,
    ..
  } = *view;
  Ok(match (pull_request, failure) {
    (Some(pull_request), _) => ReadOne::Read(Box::new(ListedPullRequest { pull_request, task })),
    (None, Some(SyncFailure::Repository(RepositoryError::PullRequestNotFound { .. }))) => ReadOne::NotFound,
    (None, Some(failure)) => ReadOne::Failed(failure),
    // A read that gives no pull request says why; without a reason GitHub had none to give.
    (None, None) => ReadOne::NotFound,
  })
}

/// Pull request `number` is not among the ones the last sync read.
fn not_synced(number: u64) -> ToolError {
  ToolError::refused(
    "pull_request_not_synced",
    vec![Message::new("pullRequests.notSynced").with("number", number.to_string())],
  )
}

/// GitHub did not give pull request `number`, for `failure`.
fn not_read(number: u64, failure: &SyncFailure) -> ToolError {
  let (code, key) = match SyncStatus::after(None, Some(failure)) {
    SyncStatus::RateLimited { .. } => ("github_rate_limited", "pullRequests.notRead.rateLimited"),
    SyncStatus::Unreachable { .. } => ("github_unreachable", "pullRequests.notRead.unreachable"),
    _ => ("pull_request_not_read", "pullRequests.notRead.failed"),
  };
  ToolError::refused(code, vec![Message::new(key).with("number", number.to_string())])
}

/// Every pull request of `linked` a search for `query` finds, most recently updated first.
fn all_rows(linked: &LinkedList, query: &str) -> Vec<ListedPullRequest> {
  linked
    .pull_requests
    .iter()
    .filter(|listed| listed.matches(query))
    .cloned()
    .collect()
}
