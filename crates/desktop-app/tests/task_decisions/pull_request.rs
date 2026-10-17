//! Create pull request on the Ready card through a fake `gh` and `git push` that answer in the shapes gh 2.97 and
//! git 2.46 write (every other git command runs for real in the seeded repository): offered beside Merge into main,
//! under way while the branch is pushed, opened as a draft with its number and Review PR, or ready for review; why it
//! is unavailable (no GitHub remote, no usable connection, another account, a missing scope, no base branch, an open
//! one already); and a refused push or a failed `gh pr create` with the recovery and Try again. GitHub is never
//! reached.

use std::{
  num::NonZeroU64,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  thread,
  time::Duration,
};

use orchester_app::{
  routes::{ProjectPage, Route},
  ui::icon::Icon,
};
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

use super::{
  Page,
  merge::{git, latest_run},
};

const READY: &str = "ready for review, merges cleanly";
const NO_BRANCH: &str = "ready for review, no branch to merge into";
const SIGNED_IN: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"repo"}]}}"#;
const OTHER_ACCOUNT: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivan-work","scopes":"repo"}]}}"#;
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"read:org"}]}}"#;
const SIGN_IN_REFUSED: &str = "fatal: could not read Username for 'https://github.com': terminal prompts disabled\n";

fn output(code: i32, stdout: &str, stderr: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  }
}

/// `gh` signed in as `auth` says, and `git push` answering `push` once `held` lets it go; every other git command
/// runs for real. Once it opened pull request 143, its list holds it.
struct FakeGitHub {
  real: ProcessRunner,
  auth: Mutex<&'static str>,
  push: Mutex<CommandOutput>,
  create: Mutex<CommandOutput>,
  held: AtomicBool,
  pushes: AtomicUsize,
  /// The arguments and standard input of the last `gh pr create`, and the branch it opened 143 from.
  created: Mutex<Option<(String, String)>>,
  opened: Mutex<Option<String>>,
}

impl FakeGitHub {
  fn new() -> Arc<Self> {
    Arc::new(Self {
      real: ProcessRunner::new(Duration::from_secs(60)),
      auth: Mutex::new(SIGNED_IN),
      push: Mutex::new(output(0, "", "")),
      create: Mutex::new(output(0, "https://github.com/ivanzaida/kontur/pull/143\n", "")),
      held: AtomicBool::new(false),
      pushes: AtomicUsize::new(0),
      created: Mutex::new(None),
      opened: Mutex::new(None),
    })
  }

  fn pushes(&self) -> usize {
    self.pushes.load(Ordering::SeqCst)
  }

  fn gh(&self, command: &Command) -> CommandOutput {
    let args = command.args().join(" ");
    match args.as_str() {
      "--version" => output(0, "gh version 2.97.0 (2026-07-31)\n", ""),
      args if args.starts_with("auth status") => output(0, &self.auth.lock(), ""),
      args if args.starts_with("pr create") => {
        let answer = self.create.lock().clone();
        *self.created.lock() = Some((args.to_owned(), command.stdin().unwrap_or_default().to_owned()));
        if answer.success() {
          let head = command.args().iter().skip_while(|arg| *arg != "--head").nth(1).cloned();
          *self.opened.lock() = head;
        }
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
fn linked(fake: &Arc<FakeGitHub>) -> Page {
  let page = Page::seeded_with_programs("decisions", Programs::with_runner(fake.clone()));
  link(&page);
  page
}

/// Presses Create pull request once whether it can be used is read, until the fake sees the push.
fn create(page: &mut Page, fake: &FakeGitHub) {
  page.window.until("the branch to be pushed", |window| {
    if let Some(mut button) = window.tree.get_element_by_id_mut("task-create-pull-request") {
      button.click();
    }
    fake.pushes() > 0
  });
}

/// The icon the button `id` shows before its label.
fn glyph(page: &mut Page, id: &str) -> String {
  page
    .window
    .tree
    .get_element_by_id(&format!("{id}-icon"))
    .and_then(|icon| icon.text_content().map(str::to_owned))
    .unwrap_or_else(|| panic!("{id} shows an icon"))
}

/// Whether *Open as draft*'s knob is at the end of its track: the switch is on.
fn knob_at_end(window: &mut super::support::Window) -> bool {
  let mut bounds = |id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (track, knob) = (
    bounds("task-pull-request-draft-mark"),
    bounds("task-pull-request-draft-knob"),
  );
  knob.x + knob.width / 2.0 > track.x + track.width / 2.0
}

#[test]
fn decision_create_pull_request_pushes_and_opens_a_draft_review_pr_opens_it() {
  let fake = FakeGitHub::new();
  let mut page = linked(&fake);
  page.open(READY, "task-create-pull-request");
  assert_eq!(page.text("task-create-pull-request-label"), "Create pull request");
  assert_eq!(page.text("task-pull-request-draft-label"), "Open as draft");
  let actions = page.text("task-ready-actions");
  assert!(
    actions.starts_with("Merge into main Create pull request Open as draft"),
    "Create pull request stands beside Merge into main: {actions}"
  );
  fake.held.store(true, Ordering::SeqCst);
  create(&mut page, &fake);
  page.window.pass();
  assert_eq!(
    glyph(&mut page, "task-create-pull-request"),
    Icon::LoaderCircle.glyph().to_string(),
    "under way while the branch is pushed"
  );
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge waits until the run's pull request is created."
  );
  fake.held.store(false, Ordering::SeqCst);
  page.until_says("task-pull-request-opened", "Pull request #143 opened as a draft.");
  assert!(!page.has("task-create-pull-request"), "a run has one pull request");
  assert!(!page.has("task-review-pull-request-reason"));
  assert!(!page.locked("task-review-pull-request"));
  let run = latest_run(&page, READY);
  let (args, body) = fake.created.lock().clone().expect("gh pr create ran");
  let branch = run.branch().as_str();
  assert!(
    args.contains(&format!("--base main --head {branch}")) && args.ends_with("--draft"),
    "{args}"
  );
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
  page.click("task-review-pull-request");
  assert_eq!(
    page.window.navigation().route_now(),
    Route::Project(
      page.project,
      ProjectPage::PullRequest(NonZeroU64::new(143).expect("a number"))
    )
  );
}

#[test]
fn decision_create_pull_request_ready_for_review_when_draft_is_off() {
  let fake = FakeGitHub::new();
  let mut page = linked(&fake);
  page.open(READY, "task-create-pull-request");
  assert!(knob_at_end(&mut page.window), "Open as draft is on by default");
  // The switch is fixed until whether a pull request can be created is read; once it can, a press turns it off.
  page.window.until("Open as draft to turn off", |window| {
    if let Some(mut switch) = window.tree.get_element_by_id_mut("task-pull-request-draft") {
      switch.click();
    }
    window.pass();
    !knob_at_end(window)
  });
  create(&mut page, &fake);
  page.until_says("task-pull-request-opened", "Pull request #143 opened.");
  let (args, _) = fake.created.lock().clone().expect("gh pr create ran");
  assert!(!args.contains("--draft"), "{args}");
}

#[test]
fn decision_create_pull_request_says_why_it_is_unavailable() {
  let fake = FakeGitHub::new();
  let mut page = Page::seeded_with_programs("decisions", Programs::with_runner(fake.clone()));
  let reopen = |page: &mut Page, words: &str| {
    page.open(NO_BRANCH, "task-merge");
    page.open(READY, "task-create-pull-request");
    page.until_says("task-create-pull-request-reason", words);
    assert!(page.locked("task-create-pull-request"));
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
  page.open(NO_BRANCH, "task-create-pull-request");
  page.until_says(
    "task-create-pull-request-reason",
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
  reopen(&mut page, "This run's branch already has open pull request #141.");
  assert!(!page.locked("task-review-pull-request"), "Review PR is offered instead");
  assert_eq!(fake.pushes(), 0, "nothing was pushed");
}

#[test]
fn decision_create_pull_request_refused_push_names_setup_git_and_tries_again() {
  let fake = FakeGitHub::new();
  *fake.push.lock() = output(128, "", SIGN_IN_REFUSED);
  let mut page = linked(&fake);
  page.open(READY, "task-create-pull-request");
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
  assert!(!page.locked("task-merge"), "Merge into main stays offered");
  assert!(page.has("task-create-pull-request"), "and so does Create pull request");
  *fake.push.lock() = output(0, "", "");
  page.window.until("Try again to open it", |window| {
    if let Some(mut retry) = window.tree.get_element_by_id_mut("task-pull-request-retry") {
      retry.click();
    }
    window.texts("task-pull-request-opened").is_some()
  });
  assert!(!page.has("task-pull-request-failure"));
}

#[test]
fn decision_create_pull_request_failed_gh_says_why_in_place() {
  let fake = FakeGitHub::new();
  *fake.create.lock() = output(
    1,
    "",
    "pull request create failed: GraphQL: No commits between main and the branch (createPullRequest)\n",
  );
  let mut page = linked(&fake);
  page.open(READY, "task-create-pull-request");
  create(&mut page, &fake);
  page.until_says("task-pull-request-failure-title", "No pull request opened");
  let branch = latest_run(&page, READY).branch().to_string();
  assert_eq!(
    page.text("task-pull-request-failure-body"),
    format!("GitHub found no commits on {branch} that main doesn't have, so there is nothing to review.")
  );
  assert!(page.has("task-pull-request-retry"));
  assert!(
    !page.has("task-pull-request-command"),
    "only a refused sign-in names gh auth setup-git"
  );
  *fake.create.lock() = output(1, "", "HTTP 502: Server Error (https://api.github.com/graphql)\n");
  page.window.until("gh's trouble", |window| {
    if let Some(mut retry) = window.tree.get_element_by_id_mut("task-pull-request-retry") {
      retry.click();
    }
    window
      .texts("task-pull-request-failure-title")
      .is_some_and(|texts| texts.join(" ") == "gh couldn't open the pull request")
  });
  assert_eq!(
    page.text("task-pull-request-failure-body"),
    "GitHub is having trouble. The branch was pushed; try again in a moment."
  );
}
