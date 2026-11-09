//! Closing, cancelling, reopening and blocking a team task through the dispatcher (decision D4, `docs/cloud-api.md`
//! "Team authority task lifecycle") against the stand-in team server, which decides each change by the caller's claim
//! and role as the contract orders: Done and the manual Blocked by the claim's holder or an Admin, Canceled, reopening
//! and Backlog ↔ Todo by an Admin, closing releasing the claim, a closed task not claimed until it is reopened. The
//! statuses a team task's lifecycle does not have, and a status on a new task, are refused here before anything is
//! sent.
//!
//! `cargo test -p orchester-workspace --test team_lifecycle team_lifecycle_`

#[path = "startup_join/support.rs"]
#[allow(dead_code, reason = "the desk's helpers the start page tests use")]
mod support;

use std::path::PathBuf;

use orchester_api::{
  Credential, SharedTask,
  client::{Client, PlainHttp, ServerAddress},
  test_support::team_server::{AuthorityClaim, AuthorityTask, CODE, Person, Standing, TeamServer, USERNAME},
  v2::{AuthorityErrorCode, Subject, TaskRead},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  secret::SecretValue,
  task::{
    TaskDetails, TaskLimits,
    organization::{Priority, TaskOrganization, WorkflowStatus},
  },
};
use orchester_workspace::{
  actions::{
    Accepted, Action, ActionError, ActionOutcome, ClaimAction, Definition, Dispatcher, Intent, LifecycleRefusal,
    Refusal, TaskAction, TeamOutcome,
  },
  sharing::{CodeRequest, Trust, team_join::JoinOutcome},
};
use support::Desk;

const YOU: AuthorityClaim = AuthorityClaim {
  holder: Person::You,
  renewed_minutes_ago: 1,
  stale: false,
  generation: 0,
};
const OLHA: AuthorityClaim = AuthorityClaim {
  holder: Person::Named("olha"),
  renewed_minutes_ago: 1,
  stale: false,
  generation: 0,
};

fn task(number: u32, workflow: &'static str, claim: Option<AuthorityClaim>) -> AuthorityTask {
  AuthorityTask {
    number,
    title: format!("Task {number}"),
    workflow,
    priority: "medium",
    pipeline: None,
    module: None,
    created_minutes_ago: 60,
    id: Some(task_id(number)),
    assignee: None,
    claim,
    work: None,
    blocked_reason: None,
  }
}

fn task_id(number: u32) -> String {
  format!("0191c7c0-0000-7000-8000-0000001{number:05x}")
}

/// A desktop joined to the stand-in team server, Kontur linked, and a dispatcher in a session of its own.
struct Team {
  desk: Desk,
  dispatcher: Dispatcher,
  project: ProjectId,
  server: TeamServer,
  team: orchester_core::id::ServerId,
}

impl Team {
  fn start(standing: Standing, tasks: Vec<AuthorityTask>) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None)]);
    server.stand_as(standing);
    server.authority_tasks(tasks);
    let desk = Desk::new();
    let address = ServerAddress::parse(&server.url()).expect("the stand-in's address");
    let request = CodeRequest {
      code: Credential::new(CODE.to_owned()),
      username: Some(USERNAME.to_owned()),
      display_name: Some("Bohdan Koval".to_owned()),
      proof: None,
    };
    let team = match desk.run(
      desk
        .join
        .join_by_code(&address, Trust::System(PlainHttp::NotAllowed), request),
    ) {
      Ok(JoinOutcome::Joined(joined)) => joined.id,
      other => panic!("not joined: {other:?}"),
    };
    let project = server.project("kontur").expect("the shared project");
    let root: PathBuf = desk.linked_project("repo-kontur", "Kontur", (team, project)).repo_root;
    let session = desk
      .run(desk.servers.team_session(team, project))
      .expect("a team session");
    let dispatcher = Dispatcher::new(root, desk.servers.clone(), Some(session));
    Self {
      desk,
      dispatcher,
      project,
      server,
      team,
    }
  }

  fn id(number: u32) -> TaskId {
    task_id(number).parse().expect("a task id")
  }

  /// The task as the server has it now.
  fn read(&self, number: u32) -> SharedTask {
    self.read_full(number).task
  }

  /// Whether the task has a claim now, its own or its module's.
  fn claimed(&self, number: u32) -> bool {
    self.read_full(number).effective_claim.is_some()
  }

  fn read_full(&self, number: u32) -> TaskRead {
    let address = ServerAddress::parse(&self.server.url()).expect("the stand-in's address");
    let client = Client::new(address, PlainHttp::NotAllowed).expect("a client");
    let token = self.desk.device_token(self.team).expect("this computer's device token");
    let token = SecretValue::new(token).expect("a token");
    self
      .desk
      .run(client.authority(&token, self.project).task(Self::id(number)))
      .expect("the task")
  }

  fn dispatch(&self, action: Action) -> Result<ActionOutcome, ActionError> {
    self.desk.run(self.dispatcher.dispatch(&Intent::new(action)))
  }

  /// The task numbered `number` changed to `edit` of its organization, on the revision read now.
  fn organize(
    &self,
    number: u32,
    edit: impl FnOnce(TaskOrganization) -> TaskOrganization,
  ) -> Result<ActionOutcome, ActionError> {
    let read = self.read(number);
    let base = read.content.organization().core().expect("an organization");
    self.dispatch(Action::Task(TaskAction::Organize {
      task: read.id,
      read: read.revision,
      edited: edit(base.clone()),
      base,
    }))
  }

  fn set(&self, number: u32, status: WorkflowStatus) -> Result<ActionOutcome, ActionError> {
    self.organize(number, |organization| organization.with_status(status))
  }

  fn block(&self, number: u32, reason: Option<&str>) -> Result<ActionOutcome, ActionError> {
    self.organize(number, |organization| {
      TaskOrganization::new(
        organization.status(),
        organization.priority(),
        reason.map(str::to_owned),
        organization.dependencies().to_vec(),
        organization.tags().to_vec(),
      )
      .expect("an organization")
    })
  }

  fn acquire(&self, number: u32) -> Result<ActionOutcome, ActionError> {
    self.dispatch(Action::Claim(ClaimAction::Acquire {
      subject: Subject::Task { id: Self::id(number) },
    }))
  }
}

/// The task the outcome says the server saved.
fn saved(outcome: Result<ActionOutcome, ActionError>) -> Box<SharedTask> {
  match outcome {
    Ok(ActionOutcome::Team(TeamOutcome::Completed(Accepted::Definition(Definition::Task(task))))) => task,
    other => panic!("not saved: {other:?}"),
  }
}

/// The refusal the outcome says the server made.
fn refusal(outcome: Result<ActionOutcome, ActionError>) -> Refusal {
  match outcome {
    Ok(ActionOutcome::Team(TeamOutcome::Rejected { refusal, .. })) => refusal,
    other => panic!("not refused: {other:?}"),
  }
}

/// A member's cancel, reopen or move is refused in the lifecycle's own words, not the team configuration's.
fn not_admin(refusal: &Refusal) -> bool {
  matches!(refusal, Refusal::TaskLifecycle(LifecycleRefusal::NotAdmin))
}

#[test]
fn team_lifecycle_the_holder_marks_done_and_the_claim_is_released() {
  let team = Team::start(Standing::Member, vec![task(1, "todo", Some(YOU))]);
  assert!(team.claimed(1));

  let done = saved(team.set(1, WorkflowStatus::Done));

  assert_eq!(done.content.workflow, WorkflowStatus::Done);
  assert!(!team.claimed(1), "closing released the claim");
  assert_eq!(team.read(1).content.workflow, WorkflowStatus::Done);
}

#[test]
fn team_lifecycle_someone_else_is_refused_and_nothing_changes() {
  let team = Team::start(
    Standing::Member,
    vec![task(1, "todo", Some(OLHA)), task(2, "todo", Some(YOU))],
  );

  // Done and Blocked are the holder's: another member meets not_holder.
  let refused = refusal(team.set(1, WorkflowStatus::Done));
  assert!(
    matches!(refused, Refusal::TaskLifecycle(LifecycleRefusal::NotHolder)),
    "{refused:?}"
  );
  let refused = refusal(team.block(1, Some("Waits for design")));
  assert!(
    matches!(refused, Refusal::TaskLifecycle(LifecycleRefusal::NotHolder)),
    "{refused:?}"
  );
  assert_eq!(team.read(1).content.workflow, WorkflowStatus::Todo);
  assert!(team.claimed(1));

  // Cancel and Backlog ↔ Todo are an Admin's, the holder's own task included.
  let refused = refusal(team.set(2, WorkflowStatus::Canceled));
  assert!(not_admin(&refused), "{refused:?}");
  let refused = refusal(team.set(2, WorkflowStatus::Backlog));
  assert!(not_admin(&refused), "{refused:?}");
  assert_eq!(team.read(2).content.workflow, WorkflowStatus::Todo);
}

#[test]
fn team_lifecycle_an_admin_cancels_and_reopens_and_a_reopened_task_is_free() {
  let team = Team::start(Standing::Admin, vec![task(1, "todo", Some(OLHA))]);

  let canceled = saved(team.set(1, WorkflowStatus::Canceled));
  assert_eq!(canceled.content.workflow, WorkflowStatus::Canceled);
  assert!(!team.claimed(1), "cancelling released olha's claim");

  // Reopened to Todo it is free, and a claim is taken as on any task.
  let reopened = saved(team.set(1, WorkflowStatus::Todo));
  assert_eq!(reopened.content.workflow, WorkflowStatus::Todo);
  assert!(!team.claimed(1));
  assert!(matches!(
    team.acquire(1),
    Ok(ActionOutcome::Team(
      TeamOutcome::Completed(_) | TeamOutcome::AcceptedPending { .. }
    ))
  ));
  assert!(team.claimed(1));
}

#[test]
fn team_lifecycle_a_closed_task_is_not_claimed_until_it_is_reopened() {
  let team = Team::start(Standing::Member, vec![task(1, "done", None)]);

  match refusal(team.acquire(1)) {
    Refusal::Command(error) => assert_eq!(error.code(), AuthorityErrorCode::TransitionDenied),
    other => panic!("not a command refusal: {other:?}"),
  }
  assert!(!team.claimed(1));
  // A member does not reopen it.
  let refused = refusal(team.set(1, WorkflowStatus::Todo));
  assert!(not_admin(&refused), "{refused:?}");
}

#[test]
fn team_lifecycle_the_holder_or_an_admin_blocks_and_clears() {
  let team = Team::start(Standing::Member, vec![task(1, "todo", Some(YOU))]);
  let blocked = saved(team.block(1, Some("Waits for design")));
  assert_eq!(blocked.content.blocked_reason.as_deref(), Some("Waits for design"));
  let cleared = saved(team.block(1, None));
  assert_eq!(cleared.content.blocked_reason, None);

  let admin = Team::start(Standing::Admin, vec![task(1, "todo", Some(OLHA))]);
  let blocked = saved(admin.block(1, Some("Waits for the API")));
  assert_eq!(blocked.content.blocked_reason.as_deref(), Some("Waits for the API"));
}

#[test]
fn team_lifecycle_an_admin_moves_a_task_between_backlog_and_todo() {
  let team = Team::start(Standing::Admin, vec![task(1, "backlog", None)]);
  assert_eq!(
    saved(team.set(1, WorkflowStatus::Todo)).content.workflow,
    WorkflowStatus::Todo
  );
  assert_eq!(
    saved(team.set(1, WorkflowStatus::Backlog)).content.workflow,
    WorkflowStatus::Backlog
  );
}

#[test]
fn team_lifecycle_a_status_the_lifecycle_does_not_have_is_refused_before_it_is_sent() {
  let team = Team::start(Standing::Admin, vec![task(1, "todo", None), task(2, "done", None)]);
  for (number, to) in [
    (1, WorkflowStatus::InProgress),
    (1, WorkflowStatus::InReview),
    (2, WorkflowStatus::Canceled),
  ] {
    let outcome = team.set(number, to);
    assert!(
      matches!(outcome, Err(ActionError::WorkflowNotOffered { to: refused, .. }) if refused == to),
      "{to:?}: {outcome:?}"
    );
  }
  assert_eq!(team.read(1).revision.get(), 1, "nothing was sent");
  assert_eq!(team.read(2).revision.get(), 1);
}

#[test]
fn team_lifecycle_a_new_team_task_carries_no_status() {
  let team = Team::start(Standing::Admin, vec![]);
  let details = TaskDetails {
    title: orchester_core::name::Name::new("New").expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let organization = TaskOrganization::default().with_status(WorkflowStatus::Done);
  let outcome = team.dispatch(Action::Task(TaskAction::Create { details, organization }));
  assert!(
    matches!(outcome, Err(ActionError::StatusOnCreate(WorkflowStatus::Done))),
    "{outcome:?}"
  );
  assert!(team.server.commands().is_empty(), "nothing was sent");
}

#[test]
fn team_lifecycle_the_servers_reasons_are_the_refusals_the_screens_name() {
  assert_eq!(
    LifecycleRefusal::of_reason("stop_run_first"),
    Some(LifecycleRefusal::StopRunFirst)
  );
  assert_eq!(
    LifecycleRefusal::of_reason("missing_reason"),
    Some(LifecycleRefusal::MissingReason)
  );
  assert_eq!(LifecycleRefusal::of_reason("anything_else"), None);
}

#[test]
fn team_lifecycle_a_member_cancelling_with_other_changes_is_refused_in_words_that_cover_both() {
  let team = Team::start(Standing::Member, vec![task(1, "todo", Some(YOU))]);

  // The server does not say which part of the patch it objected to, so a patch that also changes the priority is not
  // called a cancel's refusal.
  let refused = refusal(team.organize(1, |organization| {
    organization
      .with_status(WorkflowStatus::Canceled)
      .with_priority(Priority::High)
  }));
  assert!(
    matches!(refused, Refusal::TaskLifecycle(LifecycleRefusal::NotAdminWithDetails)),
    "{refused:?}"
  );
  let refused = refusal(team.set(1, WorkflowStatus::Canceled));
  assert!(
    not_admin(&refused),
    "the cancel alone keeps the lifecycle's words: {refused:?}"
  );
  assert_eq!(team.read(1).content.workflow, WorkflowStatus::Todo);
}

#[test]
fn team_lifecycle_a_member_without_the_claim_does_not_set_or_clear_a_blocked_reason() {
  let blocked = AuthorityTask {
    blocked_reason: Some("Waits for design"),
    ..task(3, "todo", Some(OLHA))
  };
  let team = Team::start(
    Standing::Member,
    vec![task(1, "todo", Some(OLHA)), task(2, "todo", None), blocked],
  );

  for number in [1, 2] {
    let refused = refusal(team.block(number, Some("Waits for design")));
    assert!(
      matches!(refused, Refusal::TaskLifecycle(LifecycleRefusal::NotHolder)),
      "set on task {number}: {refused:?}"
    );
    assert_eq!(team.read(number).content.blocked_reason, None);
  }
  let refused = refusal(team.block(3, None));
  assert!(
    matches!(refused, Refusal::TaskLifecycle(LifecycleRefusal::NotHolder)),
    "clear: {refused:?}"
  );
  assert_eq!(team.read(3).content.blocked_reason.as_deref(), Some("Waits for design"));
}
