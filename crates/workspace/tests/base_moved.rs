//! When the base branch moves, the working agents are told (ORC-54, `orchester_workspace::base_moved`), against a real
//! git repository: a commit on the base outside Orchester and a merge by Orchester each queue one notice for an
//! unfinished run whose branch lacks the new head, with the commits and tasks that landed and the files both sides
//! changed; moves within two minutes are one notice, which replaces the pending one; a run that has the head, or a
//! project with the switch off, is told nothing; the task's timeline shows the notice and where it stands.
//! `cargo test -p orchester-workspace --test base_moved base_moved`.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  base_move::{BaseNotice, COALESCE_MILLIS, NoticeDelivery},
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  base_moved, project_settings,
  tasks::{
    detail::TaskDetail,
    integration::merge_in,
    timeline::{ItemKind, base_moved::NoticeState},
  },
};

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A project in a git repository on `main`, whose files `shared.txt` and `other.txt` runs and the base change, with a
/// one-step pipeline its runs pin.
struct Repo {
  dir: tempfile::TempDir,
  db: ProjectDb,
  snapshot: RunSnapshot,
}

impl Repo {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-54-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("base-moved-")
      .tempdir_in(scratch)
      .expect("repo");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Base move test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("shared.txt"), "base\n").expect("shared");
    fs::write(root.join("other.txt"), "base\n").expect("other");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let snapshot = snapshot(&mut db);
    Self { dir, db, snapshot }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn task(&mut self, title: &str) -> Task {
    let details = TaskDetails {
      title: name(title),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::NONE,
    };
    self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
      .expect("task")
      .remove(0)
  }

  /// A run of `task` started from `main`'s head on `branch`, in a worktree of its own, whose step is at work after
  /// it committed `text` to `file`.
  fn working(&mut self, task: &Task, branch: &str, (file, text): (&str, &str)) -> Run {
    let base = git(self.root(), &["rev-parse", "main"]);
    let mut run = self
      .db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot: self.snapshot.clone(),
          branch: BranchName::new(branch).expect("branch"),
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
    self.db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(8)).expect("step");
    self.db.append_events(&run, &events).expect("events");
    let worktree = self.worktree(&run);
    fs::create_dir_all(worktree.parent().expect("parent")).expect("parent");
    git(
      self.root(),
      &["worktree", "add", "-b", branch, worktree.to_str().expect("path")],
    );
    fs::write(worktree.join(file), text).expect("edit");
    let subject = format!("{}: Implement (run {})", task.key(), run.number().get());
    git(&worktree, &["commit", "-am", &subject]);
    run
  }

  /// [`Self::working`], whose step then passed: the run is ready to merge.
  fn succeeded(&mut self, task: &Task, branch: &str, edit: (&str, &str)) -> Run {
    let mut run = self.working(task, branch, edit);
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.complete_step(execution, at(9)).expect("pass");
    self.db.append_events(&run, &events).expect("events");
    run
  }

  fn worktree(&self, run: &Run) -> PathBuf {
    self.root().join(".orchester/worktrees").join(run.id().to_string())
  }

  /// Commits `text` to `file` on `main` in the checkout, as a person does outside Orchester; its head.
  fn commit_on_main(&self, (file, text): (&str, &str), subject: &str) -> String {
    fs::write(self.root().join(file), text).expect("edit");
    git(self.root(), &["commit", "-am", subject]);
    git(self.root(), &["rev-parse", "main"])
  }

  /// One follow's reading of the base branches: the notices it queued.
  fn observe(&self) -> usize {
    base_moved::observe_in(self.root(), &Programs::processes()).expect("observed")
  }

  fn notices(&self, run: &Run) -> Vec<BaseNotice> {
    ProjectDb::open(self.root())
      .expect("open")
      .run_base_notices(run.id())
      .expect("notices")
  }
}

/// A pipeline of one command step, as runs pin it; what it runs does not matter here.
fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
  );
  let tool = db
    .create_resource(
      Slug::new("check-tool").expect("slug"),
      name("Check"),
      &Tool::command("", action),
      at(1),
    )
    .expect("tool");
  let tool_version = db.publish_draft::<Tool>(tool.id(), at(2)).expect("publish");
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("check").expect("key"),
    name: name("Check"),
    kind: StepKind::Tool { tool: Some(tool.id()) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let resource = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(3))
    .expect("pipeline");
  let version = db.publish_draft::<Pipeline>(resource.id(), at(4)).expect("publish");
  let tools = BTreeMap::from([(tool.id(), tool_version)]);
  let published = PublishedVersions {
    profiles: &BTreeMap::new(),
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  RunSnapshot::build(resource.id(), &version, published).expect("snapshot")
}

#[test]
fn base_moved_a_commit_outside_orchester_tells_the_run_what_landed_and_where_it_overlaps() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let other = repo.task("Pull the people's change");
  let run = repo.working(&task, "orchester/orc-1-run-1", ("shared.txt", "the run's line\n"));
  assert_eq!(repo.observe(), 0, "a branch seen the first time has not moved");
  let subject = format!("{}: Pull the people's change", other.key());
  let head = repo.commit_on_main(("shared.txt", "the base's line\n"), &subject);
  assert_eq!(repo.observe(), 1);

  let notices = repo.notices(&run);
  assert_eq!(notices.len(), 1, "{notices:?}");
  let notice = &notices[0];
  assert_eq!(notice.revision.as_str(), head);
  assert_eq!(notice.branch.as_str(), "main");
  assert_eq!(notice.commits.len(), 1);
  assert_eq!(notice.commits[0].subject, subject);
  assert_eq!(notice.tasks.len(), 1, "{:?}", notice.tasks);
  assert_eq!(notice.tasks[0].key, other.key().to_string());
  assert_eq!(notice.tasks[0].title, "Pull the people's change");
  assert_eq!(notice.overlapping, ["shared.txt"]);
  assert_eq!(notice.delivery, NoticeDelivery::Pending);
  assert_eq!(
    notice.deliver_after.unix_millis() - notice.noticed_at.unix_millis(),
    COALESCE_MILLIS,
    "it waits the window of its move out"
  );
  assert_eq!(repo.observe(), 0, "the base did not move since");

  let detail = TaskDetail::load_in(repo.root(), task.id()).expect("detail");
  let view = detail.run.as_ref().expect("the run");
  let shown: Vec<NoticeState> = orchester_workspace::tasks::timeline::base_moved::all(view)
    .filter_map(|item| match item.kind {
      ItemKind::BaseMoved(mark) => Some(mark.state),
      _ => None,
    })
    .collect();
  assert_eq!(shown, [NoticeState::WritingStep]);
}

#[test]
fn base_moved_moves_within_two_minutes_are_one_notice_that_replaces_the_pending_one() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.working(&task, "orchester/orc-1-run-1", ("shared.txt", "the run's line\n"));
  repo.observe();
  repo.commit_on_main(("other.txt", "first\n"), "First change");
  assert_eq!(repo.observe(), 1);
  let first = repo.notices(&run).remove(0);
  let head = repo.commit_on_main(("other.txt", "second\n"), "Second change");
  assert_eq!(repo.observe(), 1, "the newer move replaces the notice");

  let notices = repo.notices(&run);
  assert_eq!(notices.len(), 1, "one pending notice per run: {notices:?}");
  let notice = &notices[0];
  assert_eq!(notice.revision.as_str(), head, "about the newer head");
  let subjects: Vec<&str> = notice.commits.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(subjects, ["Second change", "First change"]);
  assert!(notice.overlapping.is_empty(), "the run changed none of their files");
  assert_eq!(
    notice.deliver_after, first.deliver_after,
    "one move: the window of the first"
  );
}

#[test]
fn base_moved_a_run_whose_branch_has_the_new_head_is_not_told() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.working(&task, "orchester/orc-1-run-1", ("shared.txt", "the run's line\n"));
  repo.observe();
  repo.commit_on_main(("other.txt", "moved\n"), "A change");
  git(&repo.worktree(&run), &["merge", "--no-edit", "main"]);
  assert_eq!(repo.observe(), 0, "its branch holds the new head already");
  assert!(repo.notices(&run).is_empty());
}

#[test]
fn base_moved_the_project_switch_off_queues_no_notice_and_on_tells_only_later_moves() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.working(&task, "orchester/orc-1-run-1", ("shared.txt", "the run's line\n"));
  repo.observe();
  assert!(
    project_settings::load_in(repo.root()).expect("settings").base_notices,
    "on by default"
  );
  project_settings::save_base_notices_in(repo.root(), false).expect("off");
  repo.commit_on_main(("other.txt", "moved\n"), "A change while off");
  assert_eq!(repo.observe(), 0);
  project_settings::save_base_notices_in(repo.root(), true).expect("on");
  assert_eq!(repo.observe(), 0, "the move seen while off is not told later");
  assert!(repo.notices(&run).is_empty());
  repo.commit_on_main(("other.txt", "moved again\n"), "A change while on");
  assert_eq!(repo.observe(), 1);
}

#[test]
fn base_moved_a_merge_by_orchester_tells_the_other_runs_still_at_work_on_the_base() {
  let mut repo = Repo::new();
  let merged_task = repo.task("Sign in");
  let working_task = repo.task("Sign out");
  let merged = repo.succeeded(&merged_task, "orchester/orc-1-run-1", ("shared.txt", "sign in\n"));
  let working = repo.working(&working_task, "orchester/orc-2-run-1", ("shared.txt", "sign out\n"));
  repo.observe();

  let programs = Programs::processes();
  merge_in(repo.root(), &programs, (merged.id(), None)).expect("merged by Orchester");
  let head = git(repo.root(), &["rev-parse", "main"]);
  let notices = repo.notices(&working);
  assert_eq!(notices.len(), 1, "the merge itself told the run: {notices:?}");
  assert_eq!(notices[0].revision.as_str(), head);
  assert_eq!(notices[0].tasks.len(), 1, "{:?}", notices[0].tasks);
  assert_eq!(notices[0].tasks[0].key, merged_task.key().to_string());
  assert_eq!(notices[0].overlapping, ["shared.txt"]);
  assert!(repo.notices(&merged).is_empty(), "the merged run is told nothing");
  assert_eq!(repo.observe(), 0, "the follow after it finds the move told");
}
