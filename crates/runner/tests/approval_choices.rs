//! The approval choices of 2026-10-02, end to end with `orchester-fake-claude` and `orchester-fake-codex`:
//! Approve similar and Always allow answer a later call of the same invocation at once, Always allow also
//! publishes the prefix in a new profile version or lists the tool on its server, Approve everything in this
//! run answers what auto-approval may, and a profile stored before approval modes that approved automatically runs
//! in Autopilot; both leave a write outside the worktree to a person. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test approval_choices approval_choices`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{ExemptTools, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, RunId},
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
  },
  resource::{AgentProfile, CommandArg, CommandProgram, StepKey},
  run::{
    AllowAutopilot, AnswerKind, Answerer, ApprovalAnswer, AutoAsk, DecidedBy, PendingToolApproval, RequestAnswer, Run,
    RunStatus,
  },
  store::{ActivityStore, McpServerStore, ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler_with, tools::TEST_COMMAND, wait_for};

/// `pencil`, with no tool listed to run without asking.
fn pencil() -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("pencil").expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(TEST_COMMAND).expect("valid program"),
      vec![CommandArg::new("--stdio").expect("valid argument")],
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("a valid server")
}

/// A Builder on `provider` that asks before MCP actions and gets `pencil`.
fn designer(provider: AgentProvider, brief: &'static str) -> TestProject {
  let commands = if provider == AgentProvider::CodexCli {
    CommandPermission::AnyInWorktree
  } else {
    CommandPermission::AskBefore
  };
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::Off);
  permissions.set_interactive_tool_approvals(true);
  project(&Setup {
    provider,
    permissions: Some(permissions),
    mcp: vec![pencil()],
    ..Setup::new(brief)
  })
}

/// A Claude Code Builder that runs `listed` commands and asks about others.
fn builder(brief: &'static str, listed: &[&str]) -> TestProject {
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(listing(listed)),
    ..Setup::new(brief)
  })
}

fn listing(listed: &[&str]) -> AgentPermissions {
  let entries = listed
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("prefix"))
    .collect();
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
    NetworkAccess::On,
  )
}

/// The Builder as stored before approval modes with *Approve automatically (don't ask me)* on, which reads as
/// Autopilot, in a project that allows it.
fn approving_automatically(brief: &'static str) -> TestProject {
  let mut stored = serde_json::to_value(listing(&["git"])).expect("stored");
  stored["auto_approve"] = serde_json::json!(true);
  let permissions: AgentPermissions = serde_json::from_value(stored).expect("read");
  assert_eq!(permissions.approval_mode(), ApprovalMode::Autopilot);
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    ..Setup::new(brief)
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.save_allow_autopilot(AllowAutopilot::ON).expect("allow autopilot");
  project
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(project: TestProject) -> Self {
    let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
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

  fn root(&self) -> &Path {
    self.project.repo.path()
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.root()).expect("open")
  }

  async fn pending(&mut self) -> PendingToolApproval {
    let root = self.project.repo.path().to_path_buf();
    let id = self.id;
    let events = &mut self.events;
    tokio::time::timeout(PATIENCE, async {
      loop {
        if let Some(request) = ProjectDb::open(&root)
          .expect("open")
          .pending_tool_approvals(id)
          .expect("requests")
          .first()
        {
          return request.clone();
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("an approval arrives")
  }

  /// Answers the first request with `answer` and waits for the run to end: every later request of the
  /// invocation is answered without a person.
  async fn answer_first(&mut self, answer: ApprovalAnswer) -> Run {
    let request = self.pending().await;
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, answer, Answerer::Person)
      .await
      .expect("answered");
    self.ended().await
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  fn decisions(&self) -> Vec<DecidedBy> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => Some(decision.decided_by),
        _ => None,
      })
      .collect()
  }

  fn readme(&self) -> String {
    let worktree = self.root().join(".orchester/worktrees").join(self.id.to_string());
    std::fs::read_to_string(worktree.join("README.md")).expect("README")
  }
}

fn approvals(readme: &str) -> usize {
  readme.lines().filter(|line| *line == "approved").count()
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_similar_answers_the_next_claude_call_at_once() {
  let mut started = Started::new(designer(AgentProvider::ClaudeCodeCli, "Change the design twice.")).await;
  let run = started.answer_first(ApprovalAnswer::Similar).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(approvals(&started.readme()), 2);
  assert_eq!(started.decisions(), [DecidedBy::PersonSimilar, DecidedBy::SimilarRule]);
  let rules = started.db().approval_rules(started.id).expect("rules");
  assert_eq!(rules.len(), 1);
  assert_eq!(rules[0].answered, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_similar_answers_the_next_codex_call_at_once() {
  let mut started = Started::new(designer(AgentProvider::CodexCli, "Design twice.")).await;
  let run = started.answer_first(ApprovalAnswer::Similar).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(approvals(&started.readme()), 2);
  assert_eq!(started.decisions(), [DecidedBy::PersonSimilar, DecidedBy::SimilarRule]);
  assert_eq!(reported(&started.db(), started.id, "mcp-2: "), ["accept"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_always_lists_the_tool_and_applies_at_once() {
  for (provider, brief) in [
    (AgentProvider::ClaudeCodeCli, "Change the design twice."),
    (AgentProvider::CodexCli, "Design twice."),
  ] {
    let mut started = Started::new(designer(provider, brief)).await;
    let run = started.answer_first(ApprovalAnswer::Always { prefix: None }).await;
    assert_eq!(run.status(), &RunStatus::Succeeded, "{provider:?}");
    assert_eq!(approvals(&started.readme()), 2, "{provider:?}");
    assert_eq!(
      started.decisions(),
      [DecidedBy::PersonAlways, DecidedBy::AlwaysAllowed],
      "{provider:?}"
    );
    let servers = started.db().mcp_servers().expect("servers");
    assert_eq!(
      servers[0].exempt_tools(),
      &ExemptTools::parse("batch_design").expect("tools"),
      "{provider:?}"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_always_publishes_the_command_prefix() {
  let mut started = Started::new(builder("Run `cargo add serde` twice.", &["git"])).await;
  let request = started.pending().await;
  assert_eq!(request.action, "cargo add serde");
  let prefix = CommandPrefix::new("cargo add").expect("prefix");
  started
    .scheduler
    .decide_tool_approval(
      started.id,
      request.id,
      ApprovalAnswer::Always {
        prefix: Some(prefix.clone()),
      },
      Answerer::Person,
    )
    .await
    .expect("answered");
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(approvals(&started.readme()), 2);
  assert_eq!(started.decisions(), [DecidedBy::PersonAlways, DecidedBy::AlwaysAllowed]);
  let profile = run
    .profile_of(&StepKey::new("implement").expect("key"))
    .expect("profile");
  let latest = started
    .db()
    .latest_version::<AgentProfile>(profile)
    .expect("read")
    .expect("published");
  assert_eq!(latest.number().get(), 2);
  let CommandPermission::Allowed(listed) = latest.body().permissions().commands() else {
    panic!("a list")
  };
  assert!(listed.entries().contains(&prefix));
  // The run keeps its pinned version and allows the prefix as a rule until it ends.
  assert_eq!(
    run.snapshot(),
    started.db().run(started.id).expect("read").expect("run").snapshot()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_approve_run_answers_later_requests() {
  let mut started = Started::new(builder("Run `cargo test -p core` twice.", &["git"])).await;
  let run = started.answer_first(ApprovalAnswer::ApproveRun).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(approvals(&started.readme()), 2);
  assert_eq!(started.decisions(), [DecidedBy::PersonRun, DecidedBy::AutoApproved]);
  assert_eq!(started.db().run_auto_approve(started.id).expect("read"), Some(true));
  assert_eq!(
    started.db().decision_counts(started.id).expect("counts").auto_approved,
    1
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_old_profile_approving_automatically_runs_in_autopilot() {
  let mut started = Started::new(approving_automatically("Run `cargo test`.")).await;
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(started.readme().contains("approved"));
  assert_eq!(started.decisions(), [DecidedBy::AutoApproved]);
  assert!(
    started
      .db()
      .pending_tool_approvals(started.id)
      .expect("requests")
      .is_empty()
  );
  let answers = started.db().request_answers(started.id).expect("answers");
  assert_eq!(
    answers[0].answer,
    Some(RequestAnswer::by(AnswerKind::Autopilot)),
    "{answers:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_old_profile_approving_automatically_still_asks_about_a_write_outside() {
  let mut started = Started::new(approving_automatically("Run `cp README.md ../outside.md`.")).await;
  let request = started.pending().await;
  assert_eq!(request.action, "cp README.md ../outside.md");
  assert!(started.decisions().is_empty());
  started
    .scheduler
    .decide_tool_approval(started.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(started.decisions(), [DecidedBy::PersonDeny]);
}

/// Automation, through Orchester's MCP server, approves once what auto-approval would approve, or denies,
/// recorded as its own answers; it cannot give a wider answer.
#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_automation_answers_once_or_denies_and_never_wider() {
  let mut started = Started::new(builder("Run `cargo test -p core` twice.", &["git"])).await;
  let first = started.pending().await;
  let wider = started
    .scheduler
    .decide_tool_approval(
      started.id,
      first.id.clone(),
      ApprovalAnswer::ApproveRun,
      Answerer::Automation,
    )
    .await;
  assert_eq!(wider, Err(Rejection::AutomationWider));
  started
    .scheduler
    .decide_tool_approval(started.id, first.id.clone(), ApprovalAnswer::Once, Answerer::Automation)
    .await
    .expect("approved once");
  let second = loop {
    let request = started.pending().await;
    if request.id != first.id {
      break request;
    }
  };
  started
    .scheduler
    .decide_tool_approval(started.id, second.id, ApprovalAnswer::Deny, Answerer::Automation)
    .await
    .expect("denied");
  let run = started.ended().await;
  assert!(run.status().is_terminal());
  assert_eq!(started.decisions(), [DecidedBy::Automation, DecidedBy::AutomationDeny]);
  let counts = started.db().decision_counts(started.id).expect("counts");
  assert_eq!((counts.by_automation, counts.denied_by_automation), (1, 1));
}

/// A credential read, which auto-approval leaves to a person: automation cannot approve it, only deny it.
#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_automation_cannot_approve_a_credential_read() {
  let mut started = Started::new(builder("Run `cp ~/.orchester-test-token token.txt`.", &["git"])).await;
  let request = started.pending().await;
  let refused = started
    .scheduler
    .decide_tool_approval(
      started.id,
      request.id.clone(),
      ApprovalAnswer::Once,
      Answerer::Automation,
    )
    .await;
  assert_eq!(refused, Err(Rejection::PersonOnly { ask: AutoAsk::Secrets }));
  assert_eq!(
    started.pending().await.id,
    request.id,
    "the request still waits for a person"
  );
  started
    .scheduler
    .decide_tool_approval(started.id, request.id, ApprovalAnswer::Deny, Answerer::Automation)
    .await
    .expect("denied");
  let run = started.ended().await;
  assert!(run.status().is_terminal());
  assert_eq!(started.decisions(), [DecidedBy::AutomationDeny]);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_a_credential_read_is_marked_as_one() {
  // A dot file of the home folder that does not exist: a credential location by where it lies.
  let mut started = Started::new(builder("Run `cp ~/.orchester-test-token token.txt`.", &["git"])).await;
  let run = started.answer_first(ApprovalAnswer::Once).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let secrets: Vec<bool> = started
    .db()
    .activity(started.id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::ToolDecided { decision, .. } => Some(decision.secrets),
      _ => None,
    })
    .collect();
  assert_eq!(secrets, [true]);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_choices_a_line_whose_parts_run_alone_runs_unasked() {
  // The fake CLI asks about any `$`; each part runs on its own: a known-safe read and a listed test.
  let brief = "Run `f=README.md; cat $f && cargo test -p core`.";
  let mut started = Started::new(builder(brief, &["cargo test"])).await;
  let run = started.ended().await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(approvals(&started.readme()), 1);
  assert_eq!(started.decisions(), [DecidedBy::Permissions]);
}
