//! Create pull request from a task's latest succeeded run, through a fake `gh` and `git` that answer in the shapes
//! gh 2.97 and git 2.46 write: whether a run can have one (no GitHub remote, no usable connection, another account,
//! a missing scope, no base branch, an open one already), creating it (the branch pushed, a draft from it into the
//! base with the task's words, then the sync that links it to the task and run), a refused push and GitHub refusing
//! to open it, and the job on the project's shared operations that does not open a second one. Before it pushes,
//! a base ahead of its remote stops it until a person creates it anyway; a base without one on the remote goes on.

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{GitHubHost, GitHubLogin},
  pull_request::{PullRequestState, PushFailure},
  repository::{RemoteName, RepositoryError},
  run::{BranchName, Run},
  store::PullRequestStore,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  activity::Activity,
  pull_requests::{
    create::{CreateProblem, Readiness, Unavailable, UnpushedBase, create_in, readiness_in},
    list::{PullRequestList, load_list_in},
  },
  tasks::integration_jobs::{IntegrationJobs, PullRequestArgs, PullRequestJob},
  workspace::OpenProject,
};
use parking_lot::Mutex;

use super::{
  link, pull_request, repo,
  support::{Fixture, at},
};

const SIGNED_IN: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"repo"}]}}"#;
const OTHER_ACCOUNT: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivan-work","scopes":"repo"}]}}"#;
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","scopes":"read:org"}]}}"#;
const ORIGIN_ON_GITHUB: &str =
  "origin\thttps://github.com/ivanzaida/kontur.git (fetch)\norigin\thttps://github.com/ivanzaida/kontur.git (push)\n";
/// The request opening a pull request on `ivanzaida/kontur`, and GitHub's answer.
const CREATE: &str = "api --method POST --hostname github.com repos/ivanzaida/kontur/pulls --input -";
const CREATED: &str = r#"{"number":143,"html_url":"https://github.com/ivanzaida/kontur/pull/143","draft":true}"#;
const VALIDATION_FAILED: &str = "gh: Validation Failed (HTTP 422)\n";

fn answer(code: i32, stdout: &str, stderr: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  }
}

/// A commit id as git prints it, on a line of its own.
fn commit(n: u32) -> String {
  format!("{n:0>40}\n")
}

/// GitHub refusing with HTTP 422 because `message`, as `gh api` writes it.
fn refused(message: &str) -> CommandOutput {
  let body = serde_json::json!({
    "message": "Validation Failed",
    "errors": [{ "resource": "PullRequest", "code": "custom", "message": message }],
  });
  answer(1, &body.to_string(), VALIDATION_FAILED)
}

/// `gh` and `git` as the person's computer has them: the sign-in `auth`, the folder's `remotes`, and what a push
/// and `gh pr create` answer. Once it opened pull request 143 from `head`, its list holds it.
struct FakeGitHub {
  auth: &'static str,
  remotes: &'static str,
  push: CommandOutput,
  create: CommandOutput,
  head: String,
  /// The commits of the local base on the run's branch that `origin/main` lacks; `None` when the remote has no
  /// `main`.
  base_ahead: Option<u64>,
  ran: Mutex<Vec<Command>>,
  opened: Mutex<bool>,
}

impl FakeGitHub {
  fn new(head: &str) -> Self {
    Self {
      auth: SIGNED_IN,
      remotes: ORIGIN_ON_GITHUB,
      push: answer(0, "", "To https://github.com/ivanzaida/kontur.git\n * [new branch]\n"),
      create: answer(0, CREATED, ""),
      head: head.to_owned(),
      base_ahead: None,
      ran: Mutex::new(Vec::new()),
      opened: Mutex::new(false),
    }
  }

  /// The argument lists of the commands that ran, in order.
  fn ran(&self) -> Vec<String> {
    self.ran.lock().iter().map(|command| command.args().join(" ")).collect()
  }

  fn ran_one(&self, start: &str) -> Option<Command> {
    self
      .ran
      .lock()
      .iter()
      .find(|command| command.args().join(" ").starts_with(start))
      .cloned()
  }

  fn open_list(&self) -> String {
    if !*self.opened.lock() {
      return "[]".to_owned();
    }
    format!(
      r#"[{{"number":143,"title":"KON-1 · Typed errors","url":"https://github.com/ivanzaida/kontur/pull/143","state":"OPEN","isDraft":true,"headRefName":"{}","baseRefName":"main","headRefOid":"{:0>40}","author":{{"login":"ivanzaida"}},"additions":1,"deletions":0,"changedFiles":1,"updatedAt":"2026-10-01T10:00:00Z","reviewDecision":"","mergedAt":null,"closedAt":null,"isCrossRepository":false}}]"#,
      self.head, 143
    )
  }
}

impl CommandRunner for FakeGitHub {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    self.ran.lock().push(command.clone());
    let args = command.args().join(" ");
    let output = match args.as_str() {
      "--version" => answer(0, "gh version 2.97.0 (2026-07-31)\n", ""),
      // The remotes are read from the repository's metadata, outside its checkout.
      args if args.starts_with("--git-dir ") && args.ends_with(" remote -v") => answer(0, self.remotes, ""),
      args if args.starts_with("auth status") => answer(0, self.auth, ""),
      args if args.starts_with("push ") => self.push.clone(),
      "fetch --quiet --no-tags --no-write-fetch-head origin +refs/heads/main:refs/remotes/origin/main"
      | "fetch --quiet --no-tags --no-write-fetch-head origin +refs/heads/staging:refs/remotes/origin/staging" => {
        answer(0, "", "")
      }
      "rev-parse --verify --quiet --end-of-options refs/remotes/origin/main^{commit}"
      | "rev-parse --verify --quiet --end-of-options refs/remotes/origin/staging^{commit}" => match self.base_ahead {
        Some(_) => answer(0, &commit(7), ""),
        None => answer(1, "", ""),
      },
      args
        if args.starts_with("merge-base --all --end-of-options refs/heads/main ")
          || args.starts_with("merge-base --all --end-of-options refs/heads/staging ") =>
      {
        answer(0, &commit(5), "")
      }
      args if args.starts_with("merge-base --is-ancestor ") => answer(0, "", ""),
      args if args.starts_with("rev-list --count ") => {
        let count = self.base_ahead.unwrap_or_default();
        answer(0, &format!("{count}\n"), "")
      }
      CREATE => {
        *self.opened.lock() |= self.create.code == Some(0);
        self.create.clone()
      }
      args if args.starts_with("pr list") && args.contains("--state open") => answer(0, &self.open_list(), ""),
      args if args.starts_with("pr list") => answer(0, "[]", ""),
      args if args.starts_with("api graphql") => answer(0, r#"{"data":{"repository":{"c0":null}}}"#, ""),
      args => panic!("`{args}` is not a command of creating a pull request"),
    };
    Ok(output)
  }
}

/// KON-1, queued with its brief and criteria; its run started on `main` and succeeded.
fn finished(fixture: &mut Fixture) -> Run {
  let task = fixture.queued_task("Typed errors", at(10));
  let mut run = fixture.started_run_from(task, (&"a".repeat(40), "main"), at(20));
  fixture.succeed(&mut run, at(30));
  run
}

fn programs(fake: &Arc<FakeGitHub>) -> Programs {
  Programs::with_runner(fake.clone())
}

#[test]
fn pull_request_readiness_says_why_a_run_cannot_have_one() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  let root = fixture.repo.path().to_path_buf();
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.remotes = "";
  let readiness = |fake: FakeGitHub| readiness_in(&root, &programs(&Arc::new(fake)), run.id()).expect("readiness");
  assert_eq!(readiness(fake), Readiness::Unavailable(Unavailable::NoGitHubRemote));
  let fake = FakeGitHub::new(run.branch().as_str());
  assert_eq!(
    readiness(fake),
    Readiness::Unavailable(Unavailable::NoConnection),
    "a GitHub remote the project is not linked to"
  );
  link(&mut fixture);
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.auth = OTHER_ACCOUNT;
  assert_eq!(
    readiness(fake),
    Readiness::Unavailable(Unavailable::WrongAccount {
      active: GitHubLogin::new("ivan-work").expect("login"),
      expected: GitHubLogin::new("ivanzaida").expect("login"),
    })
  );
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.auth = NO_REPO_SCOPE;
  assert_eq!(
    readiness(fake),
    Readiness::Unavailable(Unavailable::MissingScope {
      account: GitHubLogin::new("ivanzaida").expect("login"),
      host: GitHubHost::github_com(),
    })
  );
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.auth = r#"{"hosts":{}}"#;
  assert_eq!(
    readiness(fake),
    Readiness::Unavailable(Unavailable::NoConnection),
    "gh signed out"
  );
  assert_eq!(readiness(FakeGitHub::new(run.branch().as_str())), Readiness::Ready);
  fixture
    .db
    .replace_pull_requests(
      &repo(),
      &[pull_request(
        141,
        run.branch().as_str(),
        PullRequestState::Draft,
        None,
        900,
      )],
      at(1_000),
    )
    .expect("stored");
  assert_eq!(
    readiness(FakeGitHub::new(run.branch().as_str())),
    Readiness::Open { number: 141 }
  );
}

#[test]
fn pull_request_readiness_needs_the_branch_the_run_started_from() {
  let mut fixture = Fixture::keyed("KON");
  let task = fixture.queued_task("Typed errors", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  link(&mut fixture);
  let fake = Arc::new(FakeGitHub::new(run.branch().as_str()));
  assert_eq!(
    readiness_in(fixture.repo.path(), &programs(&fake), run.id()).expect("readiness"),
    Readiness::Unavailable(Unavailable::NoBaseBranch)
  );
  assert_eq!(
    create_in(
      fixture.repo.path(),
      &programs(&fake),
      (run.id(), true, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::Unavailable(Unavailable::NoBaseBranch))
  );
  assert!(fake.ran_one("push").is_none(), "nothing was pushed");
}

#[test]
fn pull_request_creation_pushes_the_branch_opens_a_draft_and_links_it_to_the_task() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  let fake = Arc::new(FakeGitHub::new(run.branch().as_str()));
  let created = create_in(
    fixture.repo.path(),
    &programs(&fake),
    (run.id(), true, UnpushedBase::Refuse),
  )
  .expect("created");
  assert_eq!((created.pull_request.number, created.draft), (143, true));
  let branch = run.branch().as_str();
  let push = fake.ran_one("push").expect("the branch was pushed");
  assert_eq!(
    push.args().join(" "),
    format!("push origin refs/heads/{branch}:refs/heads/{branch}")
  );
  let create = fake.ran_one(CREATE).expect("GitHub was asked to open it");
  let request: serde_json::Value =
    serde_json::from_str(create.stdin().expect("the request on standard input")).expect("JSON");
  assert_eq!(
    (&request["title"], &request["head"], &request["base"], &request["draft"]),
    (
      &serde_json::json!("KON-1 · Typed errors"),
      &serde_json::json!(branch),
      &serde_json::json!("main"),
      &serde_json::json!(true)
    )
  );
  let body = request["body"].as_str().expect("a body");
  assert!(
    body.starts_with("Sign in with a password.\n\n## Acceptance criteria\n\n1. A person can sign in"),
    "{body}"
  );
  let project = fixture.db.project().id();
  assert!(
    body.ends_with(&format!("orchester:/project/{project}/tasks/KON-1")),
    "{body}"
  );
  let ran = fake.ran();
  let pushed = ran.iter().position(|args| args.starts_with("push")).expect("pushed");
  let opened = ran.iter().position(|args| args == CREATE).expect("opened");
  assert!(
    pushed < opened,
    "the branch is on GitHub before the pull request asks for it"
  );
  assert!(
    ran[opened..].iter().any(|args| args.starts_with("pr list")),
    "a sync followed: {ran:?}"
  );
  let PullRequestList::Linked(list) = load_list_in(fixture.repo.path()).expect("the list") else {
    panic!("the project is linked");
  };
  let listed = list.find(143).expect("the sync stored it");
  let task = listed.task.as_ref().expect("linked to its task");
  assert_eq!((task.key.to_string(), task.run), ("KON-1".to_owned(), run.id()));
}

/// ORC-81: a run that started on the project's base branch `staging` asks for its pull request into `staging`, the
/// base it recorded, whatever the checkout has checked out.
#[test]
fn pull_request_creation_asks_for_the_base_branch_the_run_recorded() {
  let mut fixture = Fixture::keyed("KON");
  let task = fixture.queued_task("Typed errors", at(10));
  let mut run = fixture.started_run_from(task, (&"a".repeat(40), "staging"), at(20));
  fixture.succeed(&mut run, at(30));
  link(&mut fixture);
  let fake = Arc::new(FakeGitHub::new(run.branch().as_str()));
  create_in(
    fixture.repo.path(),
    &programs(&fake),
    (run.id(), true, UnpushedBase::Refuse),
  )
  .expect("created");
  let create = fake.ran_one(CREATE).expect("GitHub was asked to open it");
  let request: serde_json::Value =
    serde_json::from_str(create.stdin().expect("the request on standard input")).expect("JSON");
  assert_eq!(request["base"], serde_json::json!("staging"));
}

#[test]
fn pull_request_creation_stops_at_a_refused_push_and_says_why_gh_failed() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  let root = fixture.repo.path().to_path_buf();
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.push = answer(
    128,
    "",
    "fatal: could not read Username for 'https://github.com': terminal prompts disabled\n",
  );
  let fake = Arc::new(fake);
  assert!(matches!(
    create_in(&root, &programs(&fake), (run.id(), true, UnpushedBase::Refuse)),
    Err(CreateProblem::Push {
      failure: PushFailure::Authentication,
      ..
    })
  ));
  assert!(fake.ran_one(CREATE).is_none(), "GitHub was asked nothing");
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.create = refused(&format!("No commits between main and {}", run.branch()));
  assert_eq!(
    create_in(
      &root,
      &programs(&Arc::new(fake)),
      (run.id(), true, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::NoCommits)
  );
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.create = answer(1, "", "gh: Server Error (HTTP 502)\n");
  assert!(matches!(
    create_in(
      &root,
      &programs(&Arc::new(fake)),
      (run.id(), false, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::GitHub(RepositoryError::ServerError {
      status: Some(502),
      ..
    }))
  ));
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.auth = OTHER_ACCOUNT;
  let fake = Arc::new(fake);
  assert!(matches!(
    create_in(&root, &programs(&fake), (run.id(), true, UnpushedBase::Refuse)),
    Err(CreateProblem::Unavailable(Unavailable::WrongAccount { .. }))
  ));
  assert!(fake.ran_one("push").is_none(), "nothing is pushed for another account");
}

#[test]
fn pull_request_creation_refuses_a_second_one_for_the_run() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  // GitHub has one the last sync did not see; its answer names the branch, and the sync after it the number.
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.create = refused(&format!(
    "A pull request already exists for ivanzaida:{}.",
    run.branch()
  ));
  *fake.opened.lock() = true;
  assert_eq!(
    create_in(
      fixture.repo.path(),
      &programs(&Arc::new(fake)),
      (run.id(), true, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::AlreadyOpen { number: Some(143) })
  );
  fixture
    .db
    .replace_pull_requests(
      &repo(),
      &[pull_request(
        141,
        run.branch().as_str(),
        PullRequestState::Open,
        None,
        900,
      )],
      at(1_000),
    )
    .expect("stored");
  let fake = Arc::new(FakeGitHub::new(run.branch().as_str()));
  assert_eq!(
    create_in(
      fixture.repo.path(),
      &programs(&fake),
      (run.id(), true, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::AlreadyOpen { number: Some(141) })
  );
  assert!(
    fake.ran_one("push").is_none(),
    "an open one stored refuses before anything runs"
  );
}

#[test]
fn pull_request_job_runs_on_the_shared_operations_once() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  let fake = Arc::new(FakeGitHub::new(run.branch().as_str()));
  let open = OpenProject {
    project: fixture.db.project().clone(),
    repo_root: fixture.repo.path().to_path_buf(),
  };
  let jobs = IntegrationJobs::default();
  let args = PullRequestArgs {
    open,
    task: run.task(),
    key: "KON-1".into(),
    number: run.number().get(),
    run: run.id(),
    draft: true,
    unpushed: UnpushedBase::Refuse,
    programs: programs(&fake),
    activity: Activity::new(),
    changed: Arc::new(|| {}),
  };
  assert!(jobs.of(run.id()).start_pull_request(args.clone()));
  let deadline = Instant::now() + Duration::from_secs(10);
  while !matches!(jobs.state().run(run.id()).pull_request, PullRequestJob::Created(_)) {
    assert!(
      Instant::now() < deadline,
      "{:?}",
      jobs.state().run(run.id()).pull_request
    );
    std::thread::sleep(Duration::from_millis(5));
  }
  assert!(!jobs.state().run(run.id()).busy());
  assert!(!jobs.of(run.id()).start_pull_request(args), "a second one is refused");
}

#[test]
fn pull_request_creation_asks_before_including_unpushed_base_commits() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.base_ahead = Some(202);
  let fake = Arc::new(fake);
  assert_eq!(
    create_in(
      fixture.repo.path(),
      &programs(&fake),
      (run.id(), true, UnpushedBase::Refuse)
    ),
    Err(CreateProblem::BaseNotPushed {
      commits: 202,
      base: BranchName::new("main").expect("branch"),
      remote: RemoteName::origin(),
    })
  );
  assert!(
    fake.ran_one("fetch").is_some(),
    "the base is fetched before it is compared"
  );
  assert!(fake.ran_one("push").is_none(), "nothing was pushed");
  assert!(fake.ran_one(CREATE).is_none(), "GitHub was asked nothing");
  let created = create_in(
    fixture.repo.path(),
    &programs(&fake),
    (run.id(), true, UnpushedBase::Include),
  )
  .expect("created anyway");
  assert_eq!(created.pull_request.number, 143);
  let pushes = fake.ran().iter().filter(|args| args.starts_with("push ")).count();
  assert_eq!(pushes, 1, "only the run's branch was pushed, never main");
  let push = fake.ran_one("push").expect("pushed");
  assert!(!push.args().join(" ").contains("refs/heads/main"));
}

#[test]
fn pull_request_creation_goes_on_with_the_base_in_sync() {
  let mut fixture = Fixture::keyed("KON");
  let run = finished(&mut fixture);
  link(&mut fixture);
  let mut fake = FakeGitHub::new(run.branch().as_str());
  fake.base_ahead = Some(0);
  let fake = Arc::new(fake);
  let created = create_in(
    fixture.repo.path(),
    &programs(&fake),
    (run.id(), false, UnpushedBase::Refuse),
  )
  .expect("created");
  assert_eq!((created.pull_request.number, created.draft), (143, false));
  assert!(fake.ran_one("rev-list --count").is_some(), "the base was compared");
}
