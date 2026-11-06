//! A git repository whose project ran one task on a one-step pipeline whose agent step, Implement, writes: the run
//! succeeded with a checkpoint that added three functions to `src/lib.rs`, and waits for review.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  num::NonZeroU32,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  conversation::MessageBody,
  id::{ConnectionId, IdempotencyKey, ProjectId, RunId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::{Project, ProjectKey},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{Answerer, BranchName, NewRun, PublishedVersions, ReviewNote, Run, RunSnapshot, StepOutput},
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;

/// What Implement's first checkpoint leaves in `src/lib.rs`.
pub const WORK: &str = "fn one() {}\nfn two() {}\nfn three() {}\n";

pub struct Repo {
  pub repo: tempfile::TempDir,
  pub run: RunId,
  pub worktree: PathBuf,
  /// The run's head, as the person reviews it.
  pub head: Revision,
}

impl Repo {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-366-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let repo = tempfile::Builder::new()
      .prefix("review-")
      .tempdir_in(scratch)
      .expect("repo");
    git(repo.path(), &["init", "-b", "main"]);
    git(repo.path(), &["config", "user.name", "Review test"]);
    git(repo.path(), &["config", "user.email", "test@example.invalid"]);
    git(repo.path(), &["config", "core.autocrlf", "false"]);
    fs::write(repo.path().join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::create_dir_all(repo.path().join("src")).expect("src");
    fs::write(repo.path().join("src/lib.rs"), "").expect("lib");
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "base"]);
    let base = git(repo.path(), &["rev-parse", "HEAD"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("REV").expect("key"),
      name("Review"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("project");
    let snapshot = snapshot(&mut db);
    let task = db
      .create_tasks(
        IdempotencyKey::generate(),
        vec![TaskDetails {
          title: name("Add numbers"),
          brief: String::new(),
          acceptance_criteria: Vec::new(),
          pipeline: None,
          limits: TaskLimits::NONE,
        }],
        at(5),
      )
      .expect("task")
      .remove(0);
    let mut run = db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot,
          branch: BranchName::new("orchester/rev-1").expect("branch"),
          base: Revision::new(&base).expect("revision"),
          base_branch: Some(BranchName::new("main").expect("branch")),
          base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
          budget: None,
          max_duration: None,
        },
        at(6),
      )
      .expect("run");
    let events = run.start(at(7)).expect("start");
    db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(7)).expect("step");
    db.append_events(&run, &events).expect("events");
    let worktree = repo.path().join(".orchester/worktrees").join(run.id().to_string());
    fs::create_dir_all(worktree.parent().expect("parent")).expect("parent");
    git(
      repo.path(),
      &[
        "worktree",
        "add",
        "-b",
        "orchester/rev-1",
        worktree.to_str().expect("path"),
      ],
    );
    let mut fixture = Self {
      repo,
      run: run.id(),
      worktree,
      head: Revision::new(&base).expect("revision"),
    };
    fixture.head = fixture.implement(WORK, "Added one, two and three.", at(8));
    fixture
  }

  pub fn root(&self) -> &Path {
    self.repo.path()
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(self.root()).expect("project database")
  }

  pub fn stored(&self) -> Run {
    self.db().run(self.run).expect("read").expect("the run")
  }

  /// Lines `start` to `end` of `src/lib.rs` at the run's head, as the person anchors a thread to them.
  pub fn anchor(&self, start: u32, end: u32) -> (LineAnchor, Revision) {
    let lines: Vec<String> = WORK.lines().map(str::to_owned).collect();
    let excerpt = lines[start as usize - 1..end as usize].to_vec();
    let span = LineSpan::new(line(start), line(end)).expect("span");
    let anchor = LineAnchor::new("src/lib.rs", DiffSide::New, span, excerpt).expect("anchor");
    (anchor, self.head.clone())
  }

  /// Commits `content` as `src/lib.rs` in the run's worktree; the commit.
  pub fn commit(&self, content: &str) -> Revision {
    fs::write(self.worktree.join("src/lib.rs"), content).expect("write");
    git(&self.worktree, &["commit", "-am", "work"]);
    Revision::new(&git(&self.worktree, &["rev-parse", "HEAD"])).expect("revision")
  }

  /// The run's work goes back to Implement with `note`, as *Send to Implement* asks the runner, and Implement starts.
  pub fn return_to_implement(&self, note: &ReviewNote, now: Timestamp) {
    let mut db = self.db();
    let mut run = self.stored();
    let step = StepKey::new("implement").expect("key");
    let events = run
      .return_ended(&step, (note, None), Answerer::Person, now)
      .expect("returned");
    db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, now).expect("started");
    db.append_events(&run, &events).expect("events");
  }

  /// Implement, at work, commits `content`, records it as its checkpoint and ends its step with `words`, so every
  /// step passed; the commit.
  pub fn implement(&self, content: &str, words: &str, now: Timestamp) -> Revision {
    let mut db = self.db();
    let mut run = self.stored();
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let commit = self.commit(content);
    let events = run
      .record_checkpoint(execution, commit.clone(), now)
      .expect("checkpoint");
    db.append_events(&run, &events).expect("events");
    db.save_step_output(&StepOutput {
      run: run.id(),
      execution,
      step: StepKey::new("implement").expect("key"),
      outcome: None,
      text: words.to_owned(),
      unverified: Vec::new(),
      created_at: now,
    })
    .expect("output");
    let events = run.complete_step(execution, now).expect("passed");
    db.append_events(&run, &events).expect("events");
    commit
  }
}

pub fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("words")
}

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

fn line(number: u32) -> NonZeroU32 {
  NonZeroU32::new(number).expect("a line")
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// The run's snapshot: one writing agent step, Implement, on a Builder profile.
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
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(1))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(2)).expect("publish"),
  )]);
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
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
  }])
  .expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(3))
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(delivery, at(4)).expect("publish");
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  RunSnapshot::build(
    delivery,
    &version,
    PublishedVersions {
      profiles: &profiles,
      tools: &tools,
      instructions: &instructions,
      skills: &BTreeMap::new(),
    },
  )
  .expect("snapshot")
}
