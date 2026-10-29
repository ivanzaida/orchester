//! Answering a tool request by the run's approval mode end to end (`docs/design/approval-modes.md`), with
//! `orchester-fake-claude` as the agent and the reviewer: Autopilot approves what lies in the run's roots by its rule
//! without the reviewer, brings a path outside them, a push, another ref and what the step's limits forbid to the
//! person with that reason, and asks the reviewer one question about what it cannot place; Manual approves what the
//! profile allows, asks the reviewer
//! about the rest and brings what it denies, or everything when there is no reviewer, to the person; a request waiting
//! for the person when the run drops to Manual is looked at again by Manual's rules. Each request records how and why
//! it was answered. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test approval_mode_answer approval_mode_answer`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{
    AgentPermissions, AllowedCommands, ApprovalMode, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
  },
  resource::{CommandEnvironment, VariableName},
  run::{
    AllowAutopilot, AnswerKind, AnswerRecord, Answerer, ApprovalAnswer, CameToPerson, PendingToolApproval,
    PersonReason, RequestAnswer, Run,
  },
  store::SettingsStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reviewer::with_default_reviewer, scheduler_with, wait_for};

/// What the fake reviewer answers when it approves.
const APPROVES: &str = "The fake reviewer finds it serves the task.";
/// What it answers when it leaves a request to the person.
const ASKS: &str = "The fake reviewer leaves it to the person.";

/// A Claude Code Builder in `mode` whose `commands` are as given, that asks about `brief`'s command.
fn builder(brief: &'static str, mode: ApprovalMode, commands: CommandPermission) -> Setup {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::On);
  permissions.set_approval_mode(mode);
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    ..Setup::new(brief)
  }
}

/// A Builder in `mode` that asks before every command, in a project with the default reviewer.
fn asking(brief: &'static str, mode: ApprovalMode) -> TestProject {
  with_default_reviewer(project(&builder(brief, mode, CommandPermission::AskBefore)))
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
  /// Where the fake reviewer writes what it was asked.
  log: TempDir,
}

impl Started {
  /// A run of `project`'s task in a project that allows Autopilot.
  async fn new(project: TestProject) -> Self {
    let log = tempfile::tempdir().expect("a folder for the review log");
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    db.save_allow_autopilot(AllowAutopilot::ON).expect("allow autopilot");
    let variable = VariableName::new("FAKE_REVIEW_LOG").expect("name");
    let path = log.path().join("review.log").display().to_string();
    let environment = CommandEnvironment::new(BTreeMap::from([(variable, path)])).expect("environment");
    db.save_project_environment(&environment).expect("project environment");
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
      log,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  /// The first waiting request once it came to the person, with why.
  async fn came_to_person(&mut self) -> (PendingToolApproval, CameToPerson) {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(came) = db.came_to_person(id, &request.id).expect("reason")
        {
          return (request.clone(), came);
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("a request comes to the person")
  }

  async fn deny(&mut self, request: PendingToolApproval) {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
      .await
      .expect("denied");
  }

  /// How each request of the run was answered, as recorded.
  fn answers(&self) -> Vec<AnswerRecord> {
    self.db().request_answers(self.id).expect("answers")
  }

  /// What the fake reviewer wrote: `key value` lines.
  fn logged(&self, key: &str) -> Vec<String> {
    std::fs::read_to_string(self.log.path().join("review.log"))
      .unwrap_or_default()
      .lines()
      .filter_map(|line| line.strip_prefix(&format!("{key} ")).map(str::to_owned))
      .collect()
  }
}

fn answered(record: &AnswerRecord) -> Option<&RequestAnswer> {
  record.answer.as_ref()
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_autopilot_approves_inside_the_roots_by_its_rule() {
  let mut started = Started::new(asking("Run `cargo test -p core`.", ApprovalMode::Autopilot)).await;
  started.ended().await;
  let answers = started.answers();
  assert_eq!(answers.len(), 1, "{answers:?}");
  assert_eq!(answered(&answers[0]), Some(&RequestAnswer::by(AnswerKind::Autopilot)));
  assert_eq!(answers[0].came_to_person, None, "no card");
  assert!(started.logged("pid").is_empty(), "no reviewer ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_autopilot_brings_outside_push_and_other_refs_to_the_person() {
  for (brief, reason, detail) in [
    (
      "Run `cat ../../../README.md`.",
      PersonReason::Outside,
      Some("README.md"),
    ),
    ("Run `git push origin HEAD`.", PersonReason::Push, None),
    ("Run `git branch side`.", PersonReason::Refs, Some("side")),
  ] {
    let mut started = Started::new(asking(brief, ApprovalMode::Autopilot)).await;
    let (request, came) = started.came_to_person().await;
    assert_eq!(came.reason, reason, "{brief}");
    match detail {
      Some(detail) => assert!(
        came.detail.as_deref().is_some_and(|named| named.ends_with(detail)),
        "{brief}: {came:?}"
      ),
      None => assert_eq!(came.detail, None, "{brief}"),
    }
    started.deny(request).await;
    started.ended().await;
    let answers = started.answers();
    assert_eq!(
      answered(&answers[0]),
      Some(&RequestAnswer::by(AnswerKind::Person)),
      "{brief}"
    );
    assert_eq!(answers[0].came_to_person.as_ref(), Some(&came), "{brief}");
    assert!(started.logged("pid").is_empty(), "{brief}: no reviewer ran");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_autopilot_asks_the_reviewer_one_question_about_what_it_cannot_place() {
  let mut approved = Started::new(asking("Run `eval \"$CMD\"`.", ApprovalMode::Autopilot)).await;
  approved.ended().await;
  let answers = approved.answers();
  assert_eq!(answered(&answers[0]), Some(&RequestAnswer::reviewer(APPROVES)));
  assert_eq!(approved.logged("roots"), ["true"], "Autopilot's question, once");

  let mut unsure = Started::new(asking("Run `eval \"$CMD\"`. Reviewer asks.", ApprovalMode::Autopilot)).await;
  let (request, came) = unsure.came_to_person().await;
  assert_eq!(came.reason, PersonReason::ReviewerUnsure);
  assert_eq!(came.detail.as_deref(), Some(ASKS));
  unsure.deny(request).await;
  unsure.ended().await;
  assert_eq!(unsure.logged("roots"), ["true"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_autopilot_keeps_the_step_limits() {
  // A write in a step that only reads stays inside the roots, yet comes to the person without the reviewer:
  // Autopilot never approves what the step forbids. (Claude Code cannot keep the network off, so its launch records
  // it on; the network rule is checked in core's `approval_mode_answer`.)
  let brief = "Run `touch notes.md`.";
  let mut setup = builder(brief, ApprovalMode::Autopilot, CommandPermission::AskBefore);
  if let Some(permissions) = setup.permissions.as_mut() {
    permissions.set_repository(RepositoryAccess::ReadOnly);
  }
  let mut started = Started::new(with_default_reviewer(project(&setup))).await;
  let (request, came) = started.came_to_person().await;
  assert_eq!(came, CameToPerson::because(PersonReason::NotAllowed));
  started.deny(request).await;
  started.ended().await;
  assert_eq!(
    answered(&started.answers()[0]),
    Some(&RequestAnswer::by(AnswerKind::Person))
  );
  assert!(started.logged("pid").is_empty(), "no reviewer ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_manual_approves_what_the_profile_allows() {
  // The fake CLI asks about any `$`; each part runs on its own: a known-safe read and a listed test.
  let listed = AllowedCommands::new(vec![CommandPrefix::new("cargo test").expect("prefix")]).expect("list");
  let setup = builder(
    "Run `f=README.md; cat $f && cargo test -p core`.",
    ApprovalMode::Manual,
    CommandPermission::Allowed(listed),
  );
  let mut started = Started::new(with_default_reviewer(project(&setup))).await;
  started.ended().await;
  let answers = started.answers();
  assert_eq!(answered(&answers[0]), Some(&RequestAnswer::by(AnswerKind::Profile)));
  assert!(started.logged("pid").is_empty(), "no reviewer ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_manual_sends_the_rest_to_the_reviewer_then_the_person() {
  let mut approved = Started::new(asking("Run `cargo build`.", ApprovalMode::Manual)).await;
  approved.ended().await;
  assert_eq!(
    answered(&approved.answers()[0]),
    Some(&RequestAnswer::reviewer(APPROVES))
  );
  assert_eq!(approved.logged("roots"), ["false"], "Manual's question");

  let mut denied = Started::new(asking("Run `cargo build`. Reviewer asks.", ApprovalMode::Manual)).await;
  let (request, came) = denied.came_to_person().await;
  assert_eq!(came.reason, PersonReason::ReviewerDenied);
  assert_eq!(came.detail.as_deref(), Some(ASKS));
  denied.deny(request).await;
  denied.ended().await;
  assert_eq!(
    answered(&denied.answers()[0]),
    Some(&RequestAnswer::by(AnswerKind::Person))
  );

  // A hard rule keeps a credential read from the reviewer; without a reviewer every request comes to the person.
  let mut held = Started::new(asking(
    "Run `cp ~/.orchester-test-token token.txt`.",
    ApprovalMode::Manual,
  ))
  .await;
  let (request, came) = held.came_to_person().await;
  assert_eq!(came, CameToPerson::because(PersonReason::NotAllowed));
  held.deny(request).await;
  held.ended().await;
  let unreviewed = project(&builder(
    "Run `cargo build`.",
    ApprovalMode::Manual,
    CommandPermission::AskBefore,
  ));
  let mut alone = Started::new(unreviewed).await;
  let (request, came) = alone.came_to_person().await;
  assert_eq!(came, CameToPerson::because(PersonReason::NotAllowed));
  alone.deny(request).await;
  alone.ended().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_answer_drop_to_manual_looks_again_at_a_waiting_request() {
  let mut started = Started::new(asking("Run `cat ../../../README.md`.", ApprovalMode::Autopilot)).await;
  let (request, came) = started.came_to_person().await;
  assert_eq!(came.reason, PersonReason::Outside);
  started
    .db()
    .switch_run_to_manual(started.id)
    .expect("switched to Manual");
  // Manual's reviewer approves the read, which no longer waits for the person.
  let run = tokio::time::timeout(PATIENCE, started.ended())
    .await
    .expect("the run ends without the person");
  assert!(run.status().is_terminal());
  let answers = started.answers();
  assert_eq!(answers.len(), 1, "{answers:?}");
  assert_eq!(answers[0].request, request.id);
  assert_eq!(answered(&answers[0]), Some(&RequestAnswer::reviewer(APPROVES)));
  assert_eq!(answers[0].came_to_person, None, "it no longer came to the person");
  assert_eq!(started.logged("roots"), ["false"]);
}
