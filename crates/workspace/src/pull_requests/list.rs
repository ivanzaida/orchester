//! The Pull requests page of the new app and the domain tools that read pull requests (`Pull requests`
//! `l0YNS` in `orchester2.pen`): the repository's pull requests as the last sync read them, open and
//! closed, each with the task whose run made its branch ([`super::linked`]), sorted into the page's tabs,
//! searched and counted. Syncing reads the closed and merged ones too, so Merged and Closed have rows.
//! Every function blocks on SQLite or `gh`; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::GitHubLogin,
  id::ConnectionId,
  pull_request::{PullRequest, PullRequestState, ReviewDecision, SYNC_LIMIT, SyncFailure, SyncScope},
  repository::GitHubRepo,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  Inbox, InboxError, LinkedInbox, expect_active_account_in,
  linked::{LinkedTask, linked_tasks},
  status::SyncStatus,
  stored_in, sync_scope_in,
};
use crate::messages::Message;

/// A tab of the page (`Tabs` `vHiQc`): Open is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PullRequestTab {
  /// Open pull requests, drafts included.
  #[default]
  Open,
  /// Open, ready for review, and GitHub asks for a review before it can merge.
  NeedsReview,
  /// Open, and the run that made its branch is running.
  AgentWorking,
  Merged,
  /// Closed without merging.
  Closed,
}

impl PullRequestTab {
  pub const ALL: [Self; 5] = [
    Self::Open,
    Self::NeedsReview,
    Self::AgentWorking,
    Self::Merged,
    Self::Closed,
  ];

  /// The tab's name in a route's query and in the domain tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::NeedsReview => "needs_review",
      Self::AgentWorking => "agent_working",
      Self::Merged => "merged",
      Self::Closed => "closed",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  pub fn holds(self, listed: &ListedPullRequest) -> bool {
    let state = listed.pull_request.state();
    let open = matches!(state, PullRequestState::Open | PullRequestState::Draft);
    match self {
      Self::Open => open,
      Self::NeedsReview => {
        state == PullRequestState::Open && listed.pull_request.review_decision() == Some(ReviewDecision::ReviewRequired)
      }
      Self::AgentWorking => open && listed.task.as_ref().is_some_and(|task| task.agent_working),
      Self::Merged => state == PullRequestState::Merged,
      Self::Closed => state == PullRequestState::Closed,
    }
  }

  /// The tab whose rows a footer counts this tab's among: Needs your review and Agent working are
  /// parts of Open.
  const fn whole(self) -> Self {
    match self {
      Self::Open | Self::NeedsReview | Self::AgentWorking => Self::Open,
      Self::Merged => Self::Merged,
      Self::Closed => Self::Closed,
    }
  }
}

/// The order of the rows (`Sort` `E0lNS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PullRequestSort {
  #[default]
  RecentlyUpdated,
  LeastRecentlyUpdated,
  /// The highest number, the newest pull request, first.
  Number,
}

/// A pull request and the task its branch is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedPullRequest {
  pub pull_request: PullRequest,
  pub task: Option<LinkedTask>,
}

impl ListedPullRequest {
  /// Whether a search for `query` finds it: by its number, title or branches
  /// ([`PullRequest::matches`]), or by its task's key, ignoring case.
  pub fn matches(&self, query: &str) -> bool {
    let wanted = query.trim().to_lowercase();
    self.pull_request.matches(query)
      || self
        .task
        .as_ref()
        .is_some_and(|task| task.key.to_string().to_lowercase() == wanted)
  }
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestList {
  /// The project has no linked GitHub repository: pull requests are unavailable.
  Unlinked,
  Linked(Box<LinkedList>),
}

/// The linked repository's pull requests as far as they are known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedList {
  pub repo: GitHubRepo,
  pub connection: ConnectionId,
  pub expected: GitHubLogin,
  /// When the list was read; `None` before the first successful sync.
  pub synced_at: Option<Timestamp>,
  /// Why the latest sync did not read the list; `None` when it did or before it ran.
  pub failure: Option<SyncFailure>,
  /// Most recently updated first.
  pub pull_requests: Arc<Vec<ListedPullRequest>>,
}

impl LinkedList {
  pub fn status(&self) -> SyncStatus {
    SyncStatus::after(self.synced_at, self.failure.as_ref())
  }

  /// Whether syncing waits at `now`, automatically and on Refresh: until a rate limit resets.
  pub fn is_paused(&self, now: Timestamp) -> bool {
    self.failure.as_ref().is_some_and(|failure| failure.is_paused(now))
  }

  pub fn count(&self, tab: PullRequestTab) -> usize {
    self.pull_requests.iter().filter(|listed| tab.holds(listed)).count()
  }

  /// Pull request `number`, if the last sync read it.
  pub fn find(&self, number: u64) -> Option<&ListedPullRequest> {
    self
      .pull_requests
      .iter()
      .find(|listed| listed.pull_request.number() == number)
  }

  /// The rows of `tab` that a search for `query` finds, in `sort`'s order.
  pub fn rows(&self, tab: PullRequestTab, query: &str, sort: PullRequestSort) -> Vec<ListedPullRequest> {
    let mut rows: Vec<ListedPullRequest> = self
      .pull_requests
      .iter()
      .filter(|listed| tab.holds(listed) && listed.matches(query))
      .cloned()
      .collect();
    match sort {
      // The stored order.
      PullRequestSort::RecentlyUpdated => {}
      PullRequestSort::LeastRecentlyUpdated => rows.reverse(),
      PullRequestSort::Number => rows.sort_by_key(|listed| std::cmp::Reverse(listed.pull_request.number())),
    }
    rows
  }

  /// What the footer under `shown` rows of `tab` says: how many there are, or how many of their whole
  /// tab's (`2 of 6 open pull requests`), that a sync reads only the most recently updated ones when
  /// there are more, and that they are the repository's.
  pub fn footer(&self, tab: PullRequestTab, shown: usize, query: &str) -> Message {
    let whole = tab.whole();
    let total = self.count(whole);
    let part = tab != whole || !query.trim().is_empty();
    let key = match (whole, part, self.is_limited(whole), total == 1) {
      (PullRequestTab::Merged, true, ..) => "pullRequests.footer.merged.part",
      (PullRequestTab::Merged, false, true, _) => "pullRequests.footer.merged.limited",
      (PullRequestTab::Merged, false, false, true) => "pullRequests.footer.merged.one",
      (PullRequestTab::Merged, false, false, false) => "pullRequests.footer.merged.other",
      (PullRequestTab::Closed, true, ..) => "pullRequests.footer.closed.part",
      (PullRequestTab::Closed, false, true, _) => "pullRequests.footer.closed.limited",
      (PullRequestTab::Closed, false, false, true) => "pullRequests.footer.closed.one",
      (PullRequestTab::Closed, false, false, false) => "pullRequests.footer.closed.other",
      (_, true, ..) => "pullRequests.footer.open.part",
      (_, false, true, _) => "pullRequests.footer.open.limited",
      (_, false, false, true) => "pullRequests.footer.open.one",
      (_, false, false, false) => "pullRequests.footer.open.other",
    };
    Message::new(key)
      .with("shown", shown.to_string())
      .with("count", total.to_string())
      .with("limit", SYNC_LIMIT.to_string())
  }

  /// Whether the pull requests of `whole`'s read, the open ones or the closed and merged ones, filled
  /// what a sync reads, so older ones may exist that the list does not hold.
  fn is_limited(&self, whole: PullRequestTab) -> bool {
    let read = match whole {
      PullRequestTab::Merged | PullRequestTab::Closed => {
        self.count(PullRequestTab::Merged) + self.count(PullRequestTab::Closed)
      }
      _ => self.count(PullRequestTab::Open),
    };
    read >= SYNC_LIMIT as usize
  }
}

/// The pull requests as stored, without asking GitHub.
pub fn load_list_in(repo_root: &Path) -> Result<PullRequestList, InboxError> {
  list(repo_root, stored_in(repo_root)?)
}

/// Syncs the open and the closed pull requests with `programs`, stores what it read and lists it.
pub fn sync_list_in(repo_root: &Path, programs: &Programs) -> Result<PullRequestList, InboxError> {
  let inbox = sync_scope_in(repo_root, programs, SyncScope::OpenAndClosed)?;
  list(repo_root, inbox)
}

/// *Use this account*: makes the project expect the account `gh` acts as now, then syncs. `gh`
/// itself is never changed.
pub fn use_active_account_list_in(repo_root: &Path, programs: &Programs) -> Result<PullRequestList, InboxError> {
  if !expect_active_account_in(repo_root, programs)? {
    return Ok(PullRequestList::Unlinked);
  }
  sync_list_in(repo_root, programs)
}

fn list(repo_root: &Path, inbox: Inbox) -> Result<PullRequestList, InboxError> {
  let Inbox::Linked(inbox) = inbox else {
    return Ok(PullRequestList::Unlinked);
  };
  let LinkedInbox {
    repo,
    connection,
    expected,
    synced,
    failure,
  } = *inbox;
  let (synced_at, pull_requests) = match synced {
    Some(synced) => (Some(synced.synced_at), synced.pull_requests),
    None => (None, Vec::new()),
  };
  let db = ProjectDb::open(repo_root)?;
  let mut tasks = linked_tasks(&db, &pull_requests)?;
  let pull_requests = pull_requests
    .into_iter()
    .map(|pull_request| ListedPullRequest {
      task: tasks.remove(&pull_request.number()),
      pull_request,
    })
    .collect();
  Ok(PullRequestList::Linked(Box::new(LinkedList {
    repo,
    connection,
    expected,
    synced_at,
    failure,
    pull_requests: Arc::new(pull_requests),
  })))
}
