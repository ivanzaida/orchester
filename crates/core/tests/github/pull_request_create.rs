//! Opening a pull request for a run's branch: `git push` of the branch to the link's remote, then `gh pr create`,
//! against synthetic output in the shapes git 2.46 and gh 2.97 write (a sign-in git lacks cannot be produced on
//! demand, nor can GitHub's refusals): what each says, that nothing is pushed for an account the project does not
//! expect, and that no command sets an upstream, changes git config or touches gh's sign-in.

use orchester_core::{
  command::{CommandError, CommandOutput},
  connection::{GitHubHost, GitHubLogin, GitHubState},
  github::GhCli,
  id::{ConnectionId, ProjectId},
  name::Name,
  project::TaskKey,
  pull_request::{
    CreateFailure, NewPullRequest, OpenFailure, OpenedPullRequest, Push, PushFailure, gh_pr_create_command,
    git_push_command, open_pull_request, parse_gh_pr_create, parse_git_push, task_link, task_pull_request,
  },
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryError, RepositoryLink, RepositoryView},
  run::BranchName,
  task::{TaskDetails, TaskLimits},
};

use crate::{
  connection::github_connection,
  replay::{Replay, failed, fixture, gh, now, ok},
};

const VERSION: &str = "--version";
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const PUSH: &str = "push origin refs/heads/orchester/kon-12-run-1:refs/heads/orchester/kon-12-run-1";
const CREATE_DRAFT: &str = "pr create -R github.com/ivanzaida/kontur --base main --head orchester/kon-12-run-1 \
                            --title KON-12 · Add retry --body-file - --draft";

fn repo() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository")
}

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

fn new(draft: bool) -> NewPullRequest {
  NewPullRequest {
    base: branch("main"),
    head: branch("orchester/kon-12-run-1"),
    title: "KON-12 · Add retry".to_owned(),
    body: "Retry the webhook sender.\n\nTask KON-12 in Orchester: orchester:/project/p/tasks/KON-12".to_owned(),
    draft,
  }
}

fn link(expected: &str) -> RepositoryLink {
  let view = RepositoryView {
    repo: repo(),
    default_branch: branch("main"),
    permission: RepoPermission::Write,
  };
  RepositoryLink::new(
    ConnectionId::generate(),
    RemoteName::origin(),
    view,
    GitHubLogin::new(expected).expect("valid login"),
    now(),
  )
}

fn failed_with(code: i32, stderr: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: String::new(),
    stderr: stderr.to_owned(),
  }
}

#[test]
fn pr_create_opens_a_draft_from_the_head_into_the_base_with_the_body_on_standard_input() {
  let command = gh_pr_create_command(&gh(), &repo(), &new(true));
  assert_eq!(command.args().join(" "), CREATE_DRAFT);
  assert_eq!(command.stdin(), Some(new(true).body.as_str()));
  assert!(command.env().contains(&("GH_PROMPT_DISABLED".into(), "1".into())));
  let ready = gh_pr_create_command(&gh(), &repo(), &new(false));
  assert!(!ready.args().contains(&"--draft".to_owned()), "ready for review is asked for");
  assert_eq!(
    parse_gh_pr_create(&repo(), &ok(fixture!("gh_pr_create_synthetic.stdout"))),
    Ok(OpenedPullRequest {
      number: 143,
      url: "https://github.com/ivanzaida/kontur/pull/143".to_owned(),
    })
  );
}

#[test]
fn pr_create_failures_name_an_open_pull_request_no_commits_and_github_errors() {
  assert_eq!(
    parse_gh_pr_create(
      &repo(),
      &failed(fixture!("gh_pr_create_synthetic_exists.stderr"))
    ),
    Err(CreateFailure::AlreadyExists { number: Some(141) })
  );
  assert_eq!(
    parse_gh_pr_create(
      &repo(),
      &failed(fixture!("gh_pr_create_synthetic_no_commits.stderr"))
    ),
    Err(CreateFailure::NoCommits)
  );
  assert_eq!(
    parse_gh_pr_create(&repo(), &failed(fixture!("gh_repo_view_rate_limited.stderr"))),
    Err(CreateFailure::Repository(RepositoryError::RateLimited))
  );
  assert!(matches!(
    parse_gh_pr_create(&repo(), &failed(fixture!("gh_repo_view_network.stderr"))),
    Err(CreateFailure::Repository(RepositoryError::Network { .. }))
  ));
  assert!(
    matches!(
      parse_gh_pr_create(&repo(), &ok("Creating pull request…\n")),
      Err(CreateFailure::Repository(RepositoryError::Unrecognized { .. }))
    ),
    "an answer without the pull request's address opened nothing Orchester can name"
  );
  assert!(
    matches!(
      parse_gh_pr_create(&repo(), &ok("https://ghe.example.com/ivanzaida/kontur/pull/9\n")),
      Err(CreateFailure::Repository(RepositoryError::Unrecognized { .. }))
    ),
    "an address on another host is not this repository's pull request"
  );
}

#[test]
fn git_push_pushes_only_the_branch_without_prompting_or_setting_an_upstream() {
  let command = git_push_command(
    std::path::Path::new("git"),
    std::path::Path::new("repo"),
    &RemoteName::origin(),
    &branch("orchester/kon-12-run-1"),
  );
  assert_eq!(command.args().join(" "), PUSH);
  assert!(command.env().contains(&("GIT_TERMINAL_PROMPT".into(), "0".into())));
  assert!(command.env().contains(&("LC_ALL".into(), "C".into())));
  assert_eq!(parse_git_push(&ok("")), Ok(()));
}

#[test]
fn git_push_refusals_tell_a_missing_sign_in_from_a_refused_branch() {
  for stderr in [
    fixture!("git_push_synthetic_https_auth.stderr"),
    fixture!("git_push_synthetic_ssh_denied.stderr"),
    fixture!("git_push_synthetic_wrong_account.stderr"),
  ] {
    assert_eq!(
      parse_git_push(&failed_with(128, stderr)),
      Err(PushFailure::Authentication),
      "{stderr}"
    );
  }
  assert_eq!(
    parse_git_push(&failed_with(1, fixture!("git_push_synthetic_rejected.stderr"))),
    Err(PushFailure::Rejected {
      reason: "fetch first".to_owned()
    })
  );
  let Err(PushFailure::Failed { reason }) =
    parse_git_push(&failed_with(128, fixture!("git_push_synthetic_network.stderr")))
  else {
    panic!("an unreachable host is a failure");
  };
  assert!(reason.ends_with("Could not resolve host: github.com"), "{reason}");
  assert!(!reason.contains("ghp_example"), "credentials in the URL are redacted: {reason}");
}

#[test]
fn opening_pushes_the_branch_then_creates_the_pull_request() {
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (PUSH, Ok(ok(""))),
    (CREATE_DRAFT, Ok(ok(fixture!("gh_pr_create_synthetic.stdout")))),
  ]);
  let connection = github_connection();
  let github = GhCli::new(&runner, &connection);
  let push = Push {
    runner: &runner,
    git: std::path::Path::new("git"),
    repo_root: std::path::Path::new("repo"),
  };
  let attempt = open_pull_request(&github, push, (&link("ivanzaida"), &new(true)), now());
  assert_eq!(attempt.outcome.map(|opened| opened.number), Ok(143));
  let ran: Vec<String> = runner.ran().iter().map(|command| command.args().join(" ")).collect();
  assert_eq!(ran, [VERSION, AUTH_STATUS, PUSH, CREATE_DRAFT]);
}

#[test]
fn opening_pushes_nothing_for_another_account_and_stops_at_a_refused_push() {
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
  ]);
  let connection = github_connection();
  let push = Push {
    runner: &runner,
    git: std::path::Path::new("git"),
    repo_root: std::path::Path::new("repo"),
  };
  let attempt = open_pull_request(
    &GhCli::new(&runner, &connection),
    push,
    (&link("ivan-work"), &new(true)),
    now(),
  );
  assert!(matches!(
    attempt.outcome,
    Err(OpenFailure::Account(GitHubState::WrongAccount { .. }))
  ));
  assert_eq!(runner.ran().len(), 2, "only gh's check ran");
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (
      PUSH,
      Ok(failed_with(128, fixture!("git_push_synthetic_https_auth.stderr"))),
    ),
  ]);
  let push = Push { runner: &runner, ..push };
  let attempt = open_pull_request(
    &GhCli::new(&runner, &connection),
    push,
    (&link("ivanzaida"), &new(true)),
    now(),
  );
  assert_eq!(attempt.outcome, Err(OpenFailure::Push(PushFailure::Authentication)));
  let runner = Replay::new([
    (VERSION, Ok(ok(fixture!("gh_version.stdout")))),
    (AUTH_STATUS, Ok(ok(fixture!("gh_auth_status_signed_in.stdout")))),
    (PUSH, Err(CommandError::NotFound)),
  ]);
  let push = Push { runner: &runner, ..push };
  let attempt = open_pull_request(
    &GhCli::new(&runner, &connection),
    push,
    (&link("ivanzaida"), &new(true)),
    now(),
  );
  assert_eq!(
    attempt.outcome,
    Err(OpenFailure::Push(PushFailure::NotRun(CommandError::NotFound)))
  );
}

/// Orchester pushes with git's own credentials and opens the pull request with gh's own sign-in: no command
/// changes either.
#[test]
fn no_command_of_opening_changes_git_config_or_the_sign_in() {
  const FORBIDDEN: [&str; 10] = [
    "-u",
    "--set-upstream",
    "config",
    "-c",
    "credential",
    "setup-git",
    "login",
    "switch",
    "refresh",
    "--force",
  ];
  let commands = [
    git_push_command(
      std::path::Path::new("git"),
      std::path::Path::new("repo"),
      &RemoteName::origin(),
      &branch("orchester/kon-12-run-1"),
    ),
    gh_pr_create_command(&gh(), &repo(), &new(true)),
  ];
  for command in &commands {
    for arg in command.args() {
      assert!(!FORBIDDEN.contains(&arg.as_str()), "`{arg}` in {:?}", command.args());
    }
  }
}

#[test]
fn a_tasks_pull_request_says_its_key_title_summary_criteria_and_link() {
  let project = ProjectId::generate();
  let key: TaskKey = "KON-12".parse().expect("a task key");
  let details = TaskDetails {
    title: Name::new("Add retry to webhook sender").expect("valid title"),
    brief: "Retry failed webhook deliveries with backoff.\nKeep the order.\n\nBackground: the sender drops them today."
      .to_owned(),
    acceptance_criteria: vec!["Failed deliveries retry".to_owned(), "  ".to_owned(), "Order is kept".to_owned()],
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let link = task_link(project, &key);
  assert_eq!(link, format!("orchester:/project/{project}/tasks/KON-12"));
  let (title, body) = task_pull_request(&key, &details, &link);
  assert_eq!(title, "KON-12 · Add retry to webhook sender");
  assert_eq!(
    body,
    format!(
      "Retry failed webhook deliveries with backoff.\nKeep the order.\n\n## Acceptance criteria\n\n1. Failed \
       deliveries retry\n2. Order is kept\n\nTask KON-12 in Orchester: {link}"
    )
  );
  let bare = TaskDetails {
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    ..details
  };
  assert_eq!(task_pull_request(&key, &bare, &link).1, format!("Task KON-12 in Orchester: {link}"));
}
