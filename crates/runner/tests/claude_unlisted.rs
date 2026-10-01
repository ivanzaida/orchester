//! Claude Code with allowed commands, end to end ("List + ask"): the listed commands and the ones
//! Claude Code classifies as read-only run without asking; any other command, also one with a
//! `$VAR` expansion or a compound one with an unlisted part, and also in a read-only step, asks a
//! person through the permission prompt, decided once. `orchester-fake-claude` decides each call as
//! `fake_claude_rules` describes. Needs `git`. Run with `cargo test -p orchester-runner --features
//! test-agent --test claude_unlisted claude_unlisted`.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, OutsideCommands,
    RepositoryAccess,
  },
  resource::StepKey,
  run::{PendingToolApproval, Run, RunStatus, StepStatus},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, TestProject, git, pending_approval, project, reported, scheduler, wait_for};

fn listed(repository: RepositoryAccess, entries: &[&str]) -> AgentPermissions {
  let entries = entries
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("valid prefix"))
    .collect();
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("valid list")),
    NetworkAccess::Off,
  )
}

/// A run of `brief` started on Claude Code with `permissions`.
struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(brief: &'static str, permissions: AgentPermissions) -> Self {
    let project = project(&Setup {
      provider: AgentProvider::ClaudeCodeCli,
      permissions: Some(permissions),
      ..Setup::new(brief)
    });
    let (scheduler, events) = scheduler(project.repo.path());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self {
      project,
      scheduler,
      events,
      id,
    }
  }

  async fn pending(&mut self) -> PendingToolApproval {
    pending_approval(self.project.repo.path(), &mut self.events, self.id).await
  }

  async fn decide(&mut self, request: PendingToolApproval, approve: bool) -> Run {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, approve)
      .await
      .expect("decided");
    self.ended().await
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  fn reported(&self, prefix: &str) -> Vec<String> {
    reported(&self.db(), self.id, prefix)
  }

  fn readme(&self) -> String {
    std::fs::read_to_string(
      self
        .project
        .repo
        .path()
        .join(".orchester/worktrees")
        .join(self.id.to_string())
        .join("README.md"),
    )
    .expect("README")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_listed_command_runs_without_asking() {
  let mut started = Started::new("Run `git add README.md`.", listed(RepositoryAccess::Write, &["git"])).await;
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(started.reported("permission: "), ["toolu_run allow-by-rule"]);
  let argv = started.reported("argv: ").pop().expect("argv");
  for expected in [
    " --permission-mode default ",
    " --permission-prompt-tool mcp__Orchester__approve ",
    "Bash(git:*)",
    "Edit(",
  ] {
    assert!(argv.contains(expected), "{expected}: {argv}");
  }
  assert!(
    !argv.contains("Bash(rm:*)"),
    "no file command is denied by rule: {argv}"
  );
  let launched = started.db().invocations(started.id).expect("invocations")[0]
    .permissions
    .clone()
    .expect("recorded");
  assert_eq!(launched.outside_commands(), Some(OutsideCommands::PermissionPrompt));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_command_asks_and_runs_once_when_approved() {
  let mut started = Started::new("Run `pnpm build`.", listed(RepositoryAccess::Write, &["git"])).await;
  let request = started.pending().await;
  assert!(request.command);
  assert_eq!(request.action, "pnpm build");
  assert!(request.id.ends_with(":toolu_run"), "{}", request.id);
  assert_eq!(started.readme(), "hi\n", "nothing runs before the decision");
  let run = started.decide(request, true).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(started.reported("permission: "), ["toolu_run allow"]);
  assert_eq!(
    started.reported("permission again: "),
    ["toolu_run deny"],
    "the approval covers that call only"
  );
  let head = git(started.project.repo.path(), &["rev-parse", run.branch().as_str()]);
  assert_eq!(
    git(started.project.repo.path(), &["show", &format!("{head}:README.md")]),
    "hi\napproved\nhello"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_command_denied_fails_the_step() {
  let mut started = Started::new("Run `pnpm build`.", listed(RepositoryAccess::Write, &["git"])).await;
  let request = started.pending().await;
  let run = started.decide(request, false).await;
  assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  let implement = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .expect("the step ran");
  assert_eq!(implement.status(), StepStatus::Failed);
  assert!(!started.readme().contains("approved"));
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_expansions_and_compound_commands_ask() {
  for (brief, command) in [
    ("Run `git log $BRANCH`.", "git log $BRANCH"),
    ("Run `git status && pnpm build`.", "git status && pnpm build"),
    ("Run `git status; rm -rf src`.", "git status; rm -rf src"),
  ] {
    let mut started = Started::new(brief, listed(RepositoryAccess::Write, &["git"])).await;
    let request = started.pending().await;
    assert_eq!(request.action, command);
    started.decide(request, false).await;
  }
  let mut started = Started::new("Run `git status | git log`.", listed(RepositoryAccess::Write, &["git"])).await;
  assert_eq!(started.ended().await.status(), &RunStatus::Succeeded);
  assert_eq!(
    started.reported("permission: "),
    ["toolu_run allow-by-rule"],
    "a compound command whose parts are all listed runs"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_file_commands_run_listed_forms_and_ask_for_others() {
  let entries = ["git", "sed -n", "rm -f build/x"];
  for (brief, decision) in [
    ("Run `sed -n 1p README.md`.", "toolu_run allow-by-rule"),
    ("Run `rm -f build/x`.", "toolu_run allow-by-rule"),
  ] {
    let mut started = Started::new(brief, listed(RepositoryAccess::Write, &entries)).await;
    assert_eq!(started.ended().await.status(), &RunStatus::Succeeded, "{brief}");
    assert_eq!(started.reported("permission: "), [decision], "{brief}");
  }
  for (brief, command) in [
    ("Run `sed -i s/hello/bye/ README.md`.", "sed -i s/hello/bye/ README.md"),
    ("Run `rm -rf src`.", "rm -rf src"),
  ] {
    let mut started = Started::new(brief, listed(RepositoryAccess::Write, &entries)).await;
    let request = started.pending().await;
    assert_eq!(request.action, command, "{brief}");
    started.decide(request, false).await;
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn claude_unlisted_command_in_a_read_only_step_asks() {
  let mut started = Started::new("Run `pnpm build`.", listed(RepositoryAccess::ReadOnly, &["git"])).await;
  let request = started.pending().await;
  assert!(request.command);
  assert_eq!(request.action, "pnpm build");
  let argv = started.reported("argv: ").pop().expect("argv");
  for expected in [
    " --permission-mode default ",
    " --permission-prompt-tool mcp__Orchester__approve ",
    "Bash(git:*)",
    " --disallowedTools Edit Write NotebookEdit ",
  ] {
    assert!(argv.contains(expected), "{expected}: {argv}");
  }
  assert!(!argv.contains("Edit(/"), "a read-only step gets no edit rule: {argv}");
  let launched = started.db().invocations(started.id).expect("invocations")[0]
    .permissions
    .clone()
    .expect("recorded");
  assert_eq!(launched.permissions().repository(), RepositoryAccess::ReadOnly);
  assert_eq!(launched.outside_commands(), Some(OutsideCommands::PermissionPrompt));
  let run = started.decide(request, false).await;
  assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
}
