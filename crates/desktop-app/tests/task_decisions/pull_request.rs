//! Create pull request in the task header's *More actions* (ORC-731) through a fake `gh` and `git push` that answer
//! in the shapes gh 2.97 and git 2.46 write (every other git command runs for real in the seeded repository): offered
//! there with Open as draft while the run waits to be merged, under way while the branch is pushed, opened as a draft
//! with its number and Review PR on the top card, or ready for review; why it is unavailable (no GitHub remote, no
//! usable connection, another account, a missing scope, no base branch); none once the run has one; and a refused
//! push or GitHub refusing to open it, on the card with the recovery and Try again. GitHub is never reached.

use std::{
  num::NonZeroU64,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  thread,
  time::Duration,
};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{GitHubCheck, GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin, SignIn},
  diff::Changes,
  id::ConnectionId,
  name::Name,
  pull_request::{CheckRollup, PullRequest, PullRequestParts, PullRequestState},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  revision::Revision,
  run::BranchName,
  store::{ConnectionStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_store::ProjectDb;
use orchester_workspace::pull_requests::list::{PullRequestList, load_list_in};
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{
  Page, checkout_page,
  merge::{git, latest_run},
};

pub(super) const READY: &str = "ready for review, merges cleanly";
const CONFLICTS: &str = "ready for review, conflicts with main";
const NO_BRANCH: &str = "ready for review, no branch to merge into";
const SIGNED_IN: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"repo"}]}}"#;
const OTHER_ACCOUNT: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivan-work","scopes":"repo"}]}}"#;
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"read:org"}]}}"#;
const SIGN_IN_REFUSED: &str = "fatal: could not read Username for 'https://github.com': terminal prompts disabled\n";
/// The request opening a pull request on `ivanzaida/kontur`, and GitHub's answer.
const CREATE: &str = "api --method POST --hostname github.com repos/ivanzaida/kontur/pulls --input -";
const CREATED: &str = r#"{"number":143,"html_url":"https://github.com/ivanzaida/kontur/pull/143","draft":true}"#;
/// GitHub refusing a pull request without commits, as `gh api` writes it.
const NO_COMMITS: &str = r#"{"message":"Validation Failed","errors":[{"resource":"PullRequest","code":"custom","message":"No commits between main and the branch"}]}"#;

fn output(code: i32, stdout: &str, stderr: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  }
}

/// `gh` signed in as `auth` says, and `git push` answering `push` once `held` lets it go; every other git command
/// runs for real. Once it opened pull request 143, its list holds it.
pub(super) struct FakeGitHub {
  real: ProcessRunner,
  auth: Mutex<&'static str>,
  push: Mutex<CommandOutput>,
  create: Mutex<CommandOutput>,
  held: AtomicBool,
  pushes: AtomicUsize,
  /// The request of the last pull request GitHub was asked to open, and the branch it opened 143 from.
  pub(super) created: Mutex<Option<Value>>,
  opened: Mutex<Option<String>>,
}

impl FakeGitHub {
  pub(super) fn new() -> Arc<Self> {
    Arc::new(Self {
      real: ProcessRunner::new(Duration::from_secs(60)),
      auth: Mutex::new(SIGNED_IN),
      push: Mutex::new(output(0, "", "")),
      create: Mutex::new(output(0, CREATED, "")),
      held: AtomicBool::new(false),
      pushes: AtomicUsize::new(0),
      created: Mutex::new(None),
      opened: Mutex::new(None),
    })
  }

  pub(super) fn pushes(&self) -> usize {
    self.pushes.load(Ordering::SeqCst)
  }

  /// The request of the last pull request GitHub was asked to open.
  pub(super) fn request(&self) -> Value {
    self
      .created
      .lock()
      .clone()
      .expect("GitHub was asked to open a pull request")
  }

  fn gh(&self, command: &Command) -> CommandOutput {
    let args = command.args().join(" ");
    match args.as_str() {
      "--version" => output(0, "gh version 2.97.0 (2026-07-31)\n", ""),
      args if args.starts_with("auth status") => output(0, &self.auth.lock(), ""),
      CREATE => {
        let answer = self.create.lock().clone();
        let request: Value = serde_json::from_str(command.stdin().unwrap_or_default()).expect("a JSON request");
        if answer.success() {
          *self.opened.lock() = request["head"].as_str().map(str::to_owned);
        }
        *self.created.lock() = Some(request);
        answer
      }
      args if args.starts_with("pr list") && args.contains("--state open") => output(0, &self.open_list(), ""),
      args if args.starts_with("pr list") => output(0, "[]", ""),
      args if args.starts_with("api graphql") => output(0, r#"{"data":{"repository":{"c0":null}}}"#, ""),
      args => panic!("`gh {args}` is not a command of creating a pull request"),
    }
  }

  fn open_list(&self) -> String {
    let Some(head) = self.opened.lock().clone() else {
      return "[]".to_owned();
    };
    format!(
      r#"[{{"number":143,"title":"Pull request 143","url":"https://github.com/ivanzaida/kontur/pull/143","state":"OPEN","isDraft":true,"headRefName":"{head}","baseRefName":"main","headRefOid":"{:0>40}","author":{{"login":"ivanzaida"}},"additions":1,"deletions":0,"changedFiles":1,"updatedAt":"2026-10-01T10:00:00Z","reviewDecision":"","mergedAt":null,"closedAt":null,"isCrossRepository":false}}]"#,
      143
    )
  }
}

impl CommandRunner for FakeGitHub {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    if command.program().file_stem().is_some_and(|stem| stem == "gh") {
      return Ok(self.gh(command));
    }
    if command.args().first().is_some_and(|arg| arg == "push") {
      self.pushes.fetch_add(1, Ordering::SeqCst);
      while self.held.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(5));
      }
      return Ok(self.push.lock().clone());
    }
    self.real.run(command)
  }
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("a login")
}

fn repo() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("a repository")
}

/// Links the seeded project to `ivanzaida/kontur`, expecting `ivanzaida`, through a `gh` the fake answers for.
fn link(page: &Page) {
  let mut db = ProjectDb::open(&page.repo).expect("the project database");
  let mut connection = GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub · github.com").expect("a name"),
    GitHubHost::github_com(),
    page.repo.join(format!("gh{}", std::env::consts::EXE_SUFFIX)),
    Timestamp::now(),
  )
  .expect("a connection");
  connection.record_check(GitHubCheck {
    at: Timestamp::now(),
    result: GitHubCheckResult::Available {
      version: "2.97.0".into(),
      account: Some(login("ivanzaida")),
      scopes: vec!["repo".into()],
      sign_in: SignIn::SignedIn,
    },
  });
  let view = RepositoryView {
    repo: repo(),
    default_branch: BranchName::new("main").expect("a branch"),
    permission: RepoPermission::Write,
  };
  let link = RepositoryLink::new(
    connection.id(),
    RemoteName::origin(),
    view,
    login("ivanzaida"),
    Timestamp::now(),
  );
  db.create_connection(&connection.into_connection())
    .expect("the connection is saved");
  db.save_repository_link(&link).expect("the link is saved");
}

/// A linked page on the `decisions` scenario whose `gh` and `git push` are `fake`'s.
pub(super) fn linked(fake: &Arc<FakeGitHub>) -> Page {
  let page = Page::seeded_with_programs("decisions", Programs::with_runner(fake.clone()));
  link(&page);
  page
}

/// Chooses Create pull request in *More actions* once whether it can be used is read, until the fake sees a new push.
fn create(page: &mut Page, fake: &FakeGitHub) {
  let before = fake.pushes();
  page.window.until("the branch to be pushed", |window| {
    if window.texts("task-more-panel").is_none() {
      window.press("task-more");
    }
    let ready =
      window.texts("task-create-pull-request").is_some() && window.texts("task-create-pull-request-detail").is_none();
    if ready {
      window.press("task-create-pull-request");
    }
    fake.pushes() > before
  });
}

/// Waits until *More actions* says `words` under Create pull request; the menu is closed again after.
fn until_create_says(page: &mut Page, words: &str) {
  page.window.until(&format!("Create pull request: {words}"), |window| {
    if window.texts("task-more-panel").is_none() {
      window.press("task-more");
    }
    window
      .texts("task-create-pull-request-detail")
      .is_some_and(|texts| texts.join(" ").contains(words))
  });
  page.click("task-more");
}

/// Presses Try again until the failure notice's title says `title`.
fn retry_until(page: &mut Page, title: &str) {
  page.window.until(title, |window| {
    if let Some(mut retry) = window.tree.get_element_by_id_mut("task-pull-request-retry") {
      retry.click();
    }
    window
      .texts("task-pull-request-failure-title")
      .is_some_and(|texts| texts.join(" ") == title)
  });
}

#[test]
fn decision_create_pull_request_pushes_and_opens_a_draft_review_pr_opens_it() {
  let _checkout = checkout_page();
  let fake = FakeGitHub::new();
  let mut page = linked(&fake);
  page.open(READY, "task-merge");
  assert!(
    !page.has("task-create-pull-request"),
    "the top card draws only 01i's actions"
  );
  page.open_more();
  assert_eq!(page.text("task-create-pull-request-label"), "Create pull request");
  assert_eq!(page.text("task-pull-request-draft-label"), "Open as draft");
  page.click("task-more");
  fake.held.store(true, Ordering::SeqCst);
  create(&mut page, &fake);
  until_create_says(&mut page, "Under way…");
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge waits until the run's pull request is created."
  );
  fake.held.store(false, Ordering::SeqCst);
  page.until_says("task-pull-request-opened", "Pull request #143 opened as a draft.");
  assert!(!page.in_more("task-create-pull-request"), "a run has one pull request");
  assert!(!page.locked("task-review-pull-request"));
  let keys = if cfg!(target_os = "macos") {
    "⌘↵"
  } else {
    "Ctrl+Enter"
  };
  assert_eq!(
    page.text("task-ready-actions"),
    format!("Merge {keys} Review changes Close without merging Review PR"),
    "Review PR, a fourth action, once the run has a pull request"
  );
  let run = latest_run(&page, READY);
  let request = fake.request();
  assert_eq!(
    (&request["base"], &request["head"], &request["draft"]),
    (&json!("main"), &json!(run.branch().as_str()), &json!(true))
  );
  let body = request["body"].as_str().expect("a body");
  assert!(body.contains("## Acceptance criteria"), "{body}");
  let PullRequestList::Linked(list) = load_list_in(&page.repo).expect("the pull requests") else {
    panic!("the project is linked");
  };
  let linked = list
    .find(143)
    .and_then(|listed| listed.task.as_ref())
    .expect("linked to its task");
  assert_eq!(
    linked.run,
    run.id(),
    "the Pull requests list links it to the task's run"
  );
  // Another task's run, with Open as draft turned off, opens one ready for review: the row turns it off once a pull
  // request can be created.
  page.open(CONFLICTS, "task-merge");
  page.window.until("Create pull request to be offered", |window| {
    if window.texts("task-more-panel").is_none() {
      window.press("task-more");
    }
    window.texts("task-create-pull-request").is_some() && window.texts("task-create-pull-request-detail").is_none()
  });
  page.click("task-pull-request-draft");
  create(&mut page, &fake);
  page.until_says("task-pull-request-opened", "Pull request #143 opened.");
  assert_eq!(fake.request()["draft"], json!(false), "ready for review is asked for");
  // The first run keeps its pull request, which Review PR opens.
  page.open(READY, "task-pull-request-opened");
  page.click("task-review-pull-request");
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(
      page.project,
      ProjectPage::PullRequest(NonZeroU64::new(143).expect("a number"), None)
    )
  );
}

#[test]
fn decision_create_pull_request_says_why_it_is_unavailable() {
  let _checkout = checkout_page();
  let fake = FakeGitHub::new();
  let mut page = Page::seeded_with_programs("decisions", Programs::with_runner(fake.clone()));
  let reopen = |page: &mut Page, words: &str| {
    page.open(NO_BRANCH, "task-merge");
    page.open(READY, "task-merge");
    until_create_says(page, words);
  };
  reopen(&mut page, "Create pull request needs a GitHub remote");
  git(
    &page,
    &["remote", "add", "origin", "https://github.com/ivanzaida/kontur.git"],
  );
  reopen(&mut page, "Create pull request needs a usable GitHub connection");
  link(&page);
  *fake.auth.lock() = OTHER_ACCOUNT;
  reopen(&mut page, "Signed in as ivan-work, expected ivanzaida.");
  *fake.auth.lock() = NO_REPO_SCOPE;
  reopen(
    &mut page,
    "Missing scope: repo. Run gh auth refresh -h github.com -s repo",
  );
  *fake.auth.lock() = SIGNED_IN;
  page.open(NO_BRANCH, "task-merge");
  until_create_says(
    &mut page,
    "Create pull request needs the branch your checkout had when the run started",
  );
  let run = latest_run(&page, READY);
  let head = Revision::new(&"1".repeat(40)).expect("a revision");
  let parts = PullRequestParts {
    number: 141,
    title: "Already open".to_owned(),
    url: "https://github.com/ivanzaida/kontur/pull/141".to_owned(),
    state: PullRequestState::Open,
    head: run.branch().clone(),
    base: BranchName::new("main").expect("a branch"),
    checks: CheckRollup::none(head.clone()),
    head_sha: head,
    author: Some("ivanzaida".to_owned()),
    changes: Changes::default(),
    review_decision: None,
    updated_at: Timestamp::now(),
    merged_at: None,
    closed_at: None,
    cross_repository: false,
  };
  let open = PullRequest::new(&repo(), parts).expect("a pull request");
  ProjectDb::open(&page.repo)
    .expect("the project database")
    .replace_pull_requests(&repo(), &[open], Timestamp::now())
    .expect("stored");
  // The run has a pull request: the card offers Review PR, and More actions no Create pull request.
  page.open(NO_BRANCH, "task-merge");
  page.open(READY, "task-merge");
  page.window.until("Review PR", |window| {
    if window.texts("task-more-panel").is_none() {
      window.press("task-more");
    }
    window.texts("task-review-pull-request").is_some()
  });
  assert!(!page.has("task-create-pull-request"), "Review PR is offered instead");
  assert!(!page.locked("task-review-pull-request"));
  assert_eq!(fake.pushes(), 0, "nothing was pushed");
}

/// One page through every failure, so the binary's tests in parallel stay within the harness's wait: a refused push,
/// then GitHub refusing to open it twice, then Try again opening it.
#[test]
fn decision_create_pull_request_failures_say_why_in_place_and_try_again() {
  let _checkout = checkout_page();
  let fake = FakeGitHub::new();
  *fake.push.lock() = output(128, "", SIGN_IN_REFUSED);
  let mut page = linked(&fake);
  page.open(READY, "task-merge");
  create(&mut page, &fake);
  let branch = latest_run(&page, READY).branch().to_string();
  page.until_says(
    "task-pull-request-failure-title",
    &format!("git couldn't sign in to github.com to push {branch}"),
  );
  assert_eq!(
    page.text("task-pull-request-failure-body"),
    "Run gh auth setup-git or sign in to git in a terminal, then retry. Orchester doesn't change git config."
  );
  // The command row lays out each word with the space after it.
  let command = page.text("task-pull-request-command");
  assert_eq!(
    command.split_whitespace().collect::<Vec<_>>(),
    ["gh", "auth", "setup-git"]
  );
  assert!(fake.created.lock().is_none(), "GitHub was asked nothing");
  assert!(!page.locked("task-merge"), "Merge stays offered");
  assert!(
    page.in_more("task-create-pull-request"),
    "and so does Create pull request"
  );
  *fake.push.lock() = output(0, "", "");
  *fake.create.lock() = output(1, NO_COMMITS, "gh: Validation Failed (HTTP 422)\n");
  retry_until(&mut page, "No pull request opened");
  assert_eq!(
    page.text("task-pull-request-failure-body"),
    format!("GitHub found no commits on {branch} that main doesn't have, so there is nothing to review.")
  );
  assert!(
    !page.has("task-pull-request-command"),
    "only a refused sign-in names gh auth setup-git"
  );
  *fake.create.lock() = output(1, "", "gh: Server Error (HTTP 502)\n");
  retry_until(&mut page, "gh couldn't open the pull request");
  assert_eq!(
    page.text("task-pull-request-failure-body"),
    "GitHub is having trouble. The branch was pushed; try again in a moment."
  );
  *fake.create.lock() = output(0, CREATED, "");
  page.window.until("Try again to open it", |window| {
    if let Some(mut retry) = window.tree.get_element_by_id_mut("task-pull-request-retry") {
      retry.click();
    }
    window.texts("task-pull-request-opened").is_some()
  });
  assert!(!page.has("task-pull-request-failure"));
}
