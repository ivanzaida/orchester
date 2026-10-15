//! What the page shows when it cannot list pull requests as GitHub has them: no linked repository, a tab
//! without any, and each way a sync fails, the last list kept under the notice.

use orchester_core::store::PullRequestStore;
use orchester_store::ProjectDb;
use orchester_workspace::pull_requests::list::PullRequestTab;

use crate::harness::Seeded;

#[test]
fn pull_requests_without_a_linked_repository_are_unavailable() {
  let mut seeded = Seeded::new("tasks");
  seeded.open(PullRequestTab::Open);
  assert_eq!(seeded.text("pull-requests-sync-status"), "GitHub not connected");
  assert!(
    seeded
      .text("pull-requests-unavailable")
      .starts_with("Pull requests are unavailable Kontur isn’t linked to a GitHub repository."),
  );
  assert!(!seeded.shows("pull-requests-tabs"), "no tabs without a repository");
  assert!(!seeded.shows("pull-requests-refresh"), "nothing to refresh");
  seeded.window.press("pull-requests-open-connections");
  assert!(matches!(
    seeded.window.navigation().route_now(),
    orchester_app::routes::Route::Project(_, orchester_app::routes::ProjectPage::Connections(_))
  ));
}

#[test]
fn pull_requests_tab_without_any_says_so() {
  let seeded = Seeded::answering("unreachable");
  // The last sync read only #133, which asks for no review; GitHub cannot be reached now.
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database opens");
  let repo = orchester_core::repository::GitHubRepo::new(
    orchester_core::connection::GitHubHost::github_com(),
    "ivanzaida",
    "kontur",
  )
  .expect("a repository");
  let stored = db.pull_requests(&repo).expect("read").expect("a sync stored the list");
  let only: Vec<_> = stored
    .pull_requests
    .into_iter()
    .filter(|pull_request| pull_request.number() == 133)
    .collect();
  db.replace_pull_requests(&repo, &only, stored.synced_at)
    .expect("stored");
  let mut seeded = seeded;
  seeded.open(PullRequestTab::NeedsReview);
  assert_eq!(seeded.text("pull-requests-tab-needs_review"), "Needs your review 0");
  assert_eq!(seeded.text("pull-requests-no-rows-title"), "Nothing needs your review");
  assert_eq!(
    seeded.text("pull-requests-no-rows-detail"),
    "No open pull request in ivanzaida / kontur asks for your review. Review requests from GitHub appear here \
     after the next sync."
  );
}

#[test]
fn pull_requests_keep_the_list_when_github_is_unreachable() {
  let mut seeded = Seeded::answering("unreachable");
  seeded.open(PullRequestTab::Open);
  let status = seeded.text("pull-requests-sync-status");
  assert!(
    status.starts_with("Last synced ") && status.ends_with(" · GitHub unreachable"),
    "{status}"
  );
  assert!(
    seeded
      .text("pull-requests-notice")
      .starts_with("Showing pull requests from ")
  );
  assert_eq!(seeded.rows().len(), 6, "the stored list stays");
}

#[test]
fn pull_requests_name_a_github_server_error() {
  let mut seeded = Seeded::answering("server-error");
  seeded.open(PullRequestTab::Open);
  assert!(
    seeded
      .text("pull-requests-sync-status")
      .ends_with(" · GitHub is having trouble")
  );
  assert!(
    seeded
      .text("pull-requests-notice")
      .contains("GitHub is having trouble (502).")
  );
}

#[test]
fn pull_requests_rate_limit_holds_refresh_until_the_reset() {
  let mut seeded = Seeded::answering("rate-limited");
  seeded.open(PullRequestTab::Open);
  let status = seeded.text("pull-requests-sync-status");
  assert!(status.starts_with("Rate limited · resets in "), "{status}");
  assert_eq!(
    seeded.text("pull-requests-refresh-reason"),
    "Refresh is available again after the reset."
  );
  assert!(
    seeded
      .text("pull-requests-notice")
      .starts_with("GitHub rate limit reached")
  );
}

#[test]
fn pull_requests_pause_for_another_account_until_the_project_expects_it() {
  let mut seeded = Seeded::answering("wrong-account");
  seeded.open(PullRequestTab::Open);
  assert_eq!(
    seeded.text("pull-requests-sync-status"),
    "Paused · gh signed in as sam-work"
  );
  let notice = seeded.text("pull-requests-notice");
  for part in [
    "Signed in as sam-work, expected ivanzaida",
    "Check again",
    "Use this account (sam-work)",
  ] {
    assert!(notice.contains(part), "{part} in {notice}");
  }
  assert_eq!(seeded.command(), "gh auth switch -h github.com -u ivanzaida");
  seeded.window.press("pull-requests-use-account");
  seeded.settled();
  assert!(
    !seeded.shows("pull-requests-notice"),
    "the project expects sam-work now"
  );
  assert_eq!(seeded.text("pull-requests-sync-status"), "Synced just now");
}

#[test]
fn pull_requests_name_the_missing_scope_and_its_command() {
  let mut seeded = Seeded::answering("missing-scope");
  seeded.open(PullRequestTab::Open);
  let notice = seeded.text("pull-requests-notice");
  assert!(notice.starts_with("Missing scope: repo"), "{notice}");
  assert_eq!(seeded.command(), "gh auth refresh -h github.com -s repo");
  seeded.mode("ok");
  seeded.window.press("pull-requests-check-again");
  seeded.settled();
  assert!(!seeded.shows("pull-requests-notice"));
}

#[test]
fn pull_requests_without_gh_point_to_connections() {
  let mut seeded = Seeded::answering("ok");
  let program = if cfg!(windows) { "gh.cmd" } else { "gh" };
  std::fs::remove_file(seeded.bin().join(program)).expect("the fake gh is removed");
  seeded.open(PullRequestTab::Open);
  // Without its program, `gh` cannot be told apart from a program that does not answer as `gh`.
  let status = seeded.text("pull-requests-sync-status");
  assert!(status.starts_with("gh not "), "{status}");
  let notice = seeded.text("pull-requests-notice");
  assert!(notice.starts_with(&status), "{notice}");
  assert!(notice.contains("Open Connections"), "{notice}");
  assert_eq!(seeded.rows().len(), 6, "the stored list stays");
}
