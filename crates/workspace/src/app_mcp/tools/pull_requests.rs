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
      LinkedList, ListedPullRequest, PullRequestList, PullRequestSort, PullRequestTab, load_list_in, refresh_list_in,
    },
    page::{PullRequestPage, PullRequestView, refresh_page_in},
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
  /// not list (an older closed one) is then read on its own.
  #[serde(default)]
  pub refresh: bool,
}

/// A project's list, or the project as the tool names it and why its list could not be read.
type ProjectRead = Result<Read, (Value, ToolError)>;

/// A project and its list.
struct Read {
  project: OpenProject,
  list: PullRequestList,
}

pub async fn list_pull_requests(
  services: &Services,
  args: ListPullRequests,
  translate: &Translate,
) -> Result<Value, ToolError> {
  if args.refresh && args.project.is_none() {
    return Err(ToolError::InvalidArguments(
      "refresh syncs one project: name it".to_owned(),
    ));
  }
  let services = services.clone();
  let (named, refresh) = (args.project.clone(), args.refresh);
  let reads = blocking(move || match named {
    Some(named) => {
      let project = services.project(&named)?;
      let list = read(&services, &project, refresh)?;
      Ok(vec![Ok(Read { project, list })])
    }
    None => every_project(&services),
  })
  .await?;
  let query = args.search.as_deref().unwrap_or_default();
  let mut pull_requests = Vec::new();
  let mut repositories = Vec::new();
  for read in reads {
    let Read { project, list } = match read {
      Ok(read) => read,
      Err((named, error)) => {
        repositories
          .push(json!({ "project": named, "error": error.to_json(|message| translate.text(message))["error"] }));
        continue;
      }
    };
    repositories.push(repository(&project, &list));
    let PullRequestList::Linked(linked) = &list else {
      continue;
    };
    let rows = match args.tab {
      Some(tab) => linked.rows(tab.into(), query, PullRequestSort::RecentlyUpdated),
      None => all_rows(linked, query),
    };
    pull_requests.extend(rows.iter().map(|listed| item(&project, project.project.id(), listed)));
  }
  Ok(json!({ "pull_requests": pull_requests, "repositories": repositories }))
}

pub async fn get_pull_request(services: &Services, args: GetPullRequest) -> Result<Value, ToolError> {
  let services = services.clone();
  let number = args.number;
  let (project, list, listed) = blocking(move || {
    let project = services.project(&args.project)?;
    let list = read(&services, &project, args.refresh)?;
    let listed = match &list {
      PullRequestList::Unlinked => return Err(ToolError::not_found(Missing::PullRequest, format!("#{number}"))),
      PullRequestList::Linked(linked) => match linked.find(number) {
        Some(listed) => listed.clone(),
        None if args.refresh && !linked.is_paused(Timestamp::now()) => read_one(&services, &project, number)?,
        None => return Err(not_synced(number)),
      },
    };
    Ok((project, list, listed))
  })
  .await?;
  let mut found = item(&project, project.project.id(), &listed);
  found["repository"] = repository(&project, &list);
  Ok(found)
}

/// Every project the workspace knows, each read or with why it could not be.
fn every_project(services: &Services) -> Result<Vec<ProjectRead>, ToolError> {
  Ok(
    services
      .workspace
      .entries()?
      .into_iter()
      .map(|entry| {
        let named = json!({ "id": entry.project.id.to_string(), "name": entry.project.name.as_str() });
        let read = services.project(&entry.project.id.to_string()).and_then(|project| {
          let list = read(services, &project, false)?;
          Ok(Read { project, list })
        });
        read.map_err(|error| (named, error))
      })
      .collect(),
  )
}

fn read(services: &Services, project: &OpenProject, refresh: bool) -> Result<PullRequestList, ToolError> {
  let root = &project.repo_root;
  let list = if refresh {
    refresh_list_in(root, &services.programs, Timestamp::now())
  } else {
    load_list_in(root)
  };
  list.map_err(|error: InboxError| ToolError::refused("project_data", vec![error.message()]))
}

/// Pull request `number`, which the sync did not list, read on its own from GitHub.
fn read_one(services: &Services, project: &OpenProject, number: u64) -> Result<ListedPullRequest, ToolError> {
  let page = refresh_page_in(&project.repo_root, number, &services.programs)
    .map_err(|error| ToolError::refused("project_data", vec![error.message()]))?;
  let PullRequestPage::Linked(view) = page else {
    return Err(ToolError::not_found(Missing::PullRequest, format!("#{number}")));
  };
  let PullRequestView {
    pull_request,
    task,
    failure,
    ..
  } = *view;
  match (pull_request, failure) {
    (Some(pull_request), _) => Ok(ListedPullRequest { pull_request, task }),
    (None, Some(SyncFailure::Repository(RepositoryError::PullRequestNotFound { .. }))) => {
      Err(ToolError::not_found(Missing::PullRequest, format!("#{number}")))
    }
    (None, _) => Err(not_synced(number)),
  }
}

/// Pull request `number` is not among the ones the last sync read.
fn not_synced(number: u64) -> ToolError {
  ToolError::refused(
    "pull_request_not_synced",
    vec![Message::new("pullRequests.notSynced").with("number", number.to_string())],
  )
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
