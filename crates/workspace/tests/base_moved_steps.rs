//! Which step of a run takes a notice that its base branch moved, as the task page tells it (ORC-65): in Implement →
//! Verify, where Verify writes too (build output and QA files), a notice waits for Implement while Implement is ahead
//! or at work; once only Verify is left it waits until the work goes back to Implement, and *Ask an agent to resolve*
//! picks Implement, not the last writing step. Run with
//! `cargo test -p orchester-workspace --test base_moved_steps base_moved_steps`.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  num::NonZeroU32,
};

use orchester_core::{
  base_move::{BaseNotice, LandedCommit, NoticeDelivery},
  id::{ConnectionId, IdempotencyKey, NoticeId, ProjectId, TaskId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  detail::TaskDetail,
  return_work::implementing_step,
  timeline::{ItemKind, base_moved::NoticeState},
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// A project whose pipeline is Implement → Verify, both writing on a Builder that writes, Verify returning its
/// failures to Implement; and a run of a task on it, started, with Implement at work.
struct Verified {
  repo: tempfile::TempDir,
  db: ProjectDb,
  task: TaskId,
  run: Run,
}

fn step(key: &str, title: &str, builder: orchester_core::id::ResourceId) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish"),
  )]);
  let verify = Step {
    depends_on: BTreeSet::from([StepKey::new("implement").expect("key")]),
    on_failure: Some(FailureRoute {
      return_to: StepKey::new("implement").expect("key"),
      max_retries: NonZeroU32::new(2).expect("positive"),
    }),
    ..step("verify", "Verify", builder)
  };
  let pipeline = Pipeline::new(vec![step("implement", "Implement", builder), verify]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(4))
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(delivery, at(5)).expect("publish");
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  RunSnapshot::build(delivery, &version, published).expect("snapshot")
}

impl Verified {
  fn new() -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("project");
    let snapshot = snapshot(&mut db);
    let details = TaskDetails {
      title: name("Sign in"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::NONE,
    };
    let task = db
      .create_tasks(IdempotencyKey::generate(), vec![details], at(6))
      .expect("task")[0]
      .id();
    let new = NewRun {
      task,
      snapshot,
      branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: Some(BranchName::new("main").expect("branch")),
      base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
      budget: None,
      max_duration: None,
    };
    let mut run = db.create_run(IdempotencyKey::generate(), new, at(7)).expect("run");
    let events = run.start(at(8)).expect("start");
    db.append_events(&run, &events).expect("events");
    let mut project = Self { repo, db, task, run };
    project.start_step(0, at(9));
    project
  }

  fn execution(&self, index: usize) -> orchester_core::id::StepExecutionId {
    self.run.current_attempt().expect("attempt").executions()[index].id()
  }

  fn start_step(&mut self, index: usize, now: Timestamp) {
    let events = self.run.start_step(self.execution(index), now).expect("start step");
    self.db.append_events(&self.run, &events).expect("events");
  }

  fn pass_step(&mut self, index: usize, now: Timestamp) {
    let events = self.run.complete_step(self.execution(index), now).expect("pass");
    self.db.append_events(&self.run, &events).expect("events");
  }

  /// Queues a notice that `main` moved, as the desktop does.
  fn moved(&mut self) {
    let revision = Revision::new(&"b".repeat(40)).expect("revision");
    let notice = BaseNotice {
      id: NoticeId::generate(),
      run: self.run.id(),
      task: self.task,
      branch: BranchName::new("main").expect("branch"),
      revision: revision.clone(),
      commits: vec![LandedCommit {
        revision,
        subject: "A change".to_owned(),
      }],
      tasks: Vec::new(),
      overlapping: Vec::new(),
      noticed_at: at(10),
      deliver_after: at(10),
      delivery: NoticeDelivery::Pending,
    };
    self.db.queue_base_notice(&notice).expect("queued");
  }

  fn detail(&self) -> TaskDetail {
    TaskDetail::load_in(self.repo.path(), self.task).expect("the task is read")
  }

  fn shown(&self) -> Vec<NoticeState> {
    let detail = self.detail();
    let view = detail.run.as_ref().expect("the run");
    orchester_workspace::tasks::timeline::base_moved::all(view)
      .filter_map(|item| match item.kind {
        ItemKind::BaseMoved(mark) => Some(mark.state),
        _ => None,
      })
      .collect()
  }
}

#[test]
fn base_moved_steps_a_notice_waits_for_implement_then_until_the_work_goes_back_to_it() {
  let mut project = Verified::new();
  project.moved();
  assert_eq!(project.shown(), [NoticeState::WritingStep], "Implement is at work");

  project.pass_step(0, at(11));
  project.start_step(1, at(12));
  assert_eq!(
    project.shown(),
    [NoticeState::WorkGoesBack {
      step: "Implement".into()
    }],
    "only Verify is left, and it never takes the notice though it writes"
  );
}

#[test]
fn base_moved_steps_resolving_a_conflict_goes_to_implement_not_the_last_writing_step() {
  let mut project = Verified::new();
  project.pass_step(0, at(11));
  project.start_step(1, at(12));
  project.pass_step(1, at(13));
  let detail = project.detail();
  let view = detail.run.as_ref().expect("the run");
  assert_eq!(
    implementing_step(view),
    Some(StepKey::new("implement").expect("key")),
    "Verify writes, but it merges no base"
  );
}
