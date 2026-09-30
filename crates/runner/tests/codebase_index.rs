//! The codebase index of a run's base revision, end to end against `orchester-fake-codex` and
//! `orchester-fake-claude`: built before the first step, removed with the worktree that used it, and
//! given to new and resumed sessions with the internal `codebase-map` skill and ripgrep first on the
//! agent's `PATH`; nothing while the project has it off; reported to the agent when it cannot be
//! built or ripgrep is missing, while the run goes on; and the tokens used before the first edit.
//! ripgrep is looked up on the `PATH` the project's environment gives the agent, so a test decides
//! whether one is found. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test codebase_index`.

mod support;

use std::{collections::BTreeMap, fs, num::NonZeroU32, path::Path, time::Duration};

use orchester_core::{
  code_index::{CodeIndexSetting, CodeIndexUnavailable, CodeIndexUse},
  connection::{
    AgentProvider,
    stream::{AgentEvent, RunnerProblem},
  },
  id::{IdempotencyKey, RunId},
  integration::{Leftovers, leftovers},
  resource::{CommandEnvironment, VariableName},
  run::{Run, RunStatus, TokenUsage},
  store::{ActivityStore, SettingsStore},
};
use orchester_exec::{ProcessRunner, ripgrep};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;
use tempfile::TempDir;

use self::support::{Setup, git, killed_mid_run, project, reported, scheduler, wait_for};

const FAKE_RIPGREP: &str = "not really ripgrep";

fn indexed(brief: &'static str) -> Setup {
  Setup {
    code_index: CodeIndexSetting::On,
    ..Setup::new(brief)
  }
}

fn index_of(root: &Path, run: &Run) -> std::path::PathBuf {
  root.join(".orchester").join("index").join(run.base().as_str())
}

fn left_by(root: &Path, run: &Run) -> Leftovers {
  let git = ProcessRunner::new(Duration::from_secs(30));
  leftovers(&git, Path::new("git"), root, run).expect("what the run left")
}

/// Gives the project's agents a `PATH` of one folder, holding a ripgrep when `with_ripgrep`.
fn agent_path(root: &Path, with_ripgrep: bool) -> TempDir {
  let folder = tempfile::tempdir().expect("a folder for the PATH");
  if with_ripgrep {
    fs::write(folder.path().join(ripgrep::PROGRAM), FAKE_RIPGREP).expect("a ripgrep");
  }
  let path = folder.path().to_str().expect("a Unicode temporary path").to_owned();
  let environment = CommandEnvironment::new(BTreeMap::from([(VariableName::new("PATH").expect("name"), path)]))
    .expect("valid variables");
  ProjectDb::open(root)
    .expect("open")
    .save_project_environment(&environment)
    .expect("save the project's environment");
  folder
}

/// Runs the task until its run ends or waits for a person.
async fn run_to_the_end(root: &Path, task: orchester_core::id::TaskId) -> Run {
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| {
    run.status().is_terminal() || run.waits_for_a_person()
  })
  .await
}

fn problems(root: &Path, run: RunId) -> Vec<RunnerProblem> {
  ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect()
}

/// What each invocation of `run` got of the code index, in launch order.
fn uses(root: &Path, run: RunId) -> Vec<CodeIndexUse> {
  let db = ProjectDb::open(root).expect("open");
  db.invocations(run)
    .expect("invocations")
    .iter()
    .map(|invocation| {
      db.invocation_code_index(invocation.id)
        .expect("read")
        .expect("recorded")
    })
    .collect()
}

/// The prompt lines of `run`'s invocations that start with `prefix`.
fn prompt_lines(root: &Path, run: RunId, prefix: &str) -> Vec<String> {
  let db = ProjectDb::open(root).expect("open");
  reported(&db, run, "prompt| ")
    .into_iter()
    .filter(|line| line.starts_with(prefix))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_is_built_for_the_base_revision_before_the_first_step() {
  let project = project(&indexed("Say hello."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  let ended = run_to_the_end(root, project.task).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);

  let index = index_of(root, &ended);
  assert_eq!(
    fs::read_to_string(index.join("tree.txt")).expect("the tree"),
    "README.md\tmarkdown\t1\n"
  );
  let manifest = fs::read_to_string(index.join("manifest.json")).expect("the manifest");
  assert!(manifest.contains(ended.base().as_str()), "{manifest}");
  assert!(problems(root, ended.id()).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_goes_with_the_worktree_removed_before_running_again() {
  let project = project(&indexed("Please fail."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  let (scheduler, mut events) = scheduler(root);
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let failed = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  let first_index = index_of(root, &failed);
  assert!(first_index.is_dir());
  fs::write(root.join("README.md"), "changed\n").expect("change the checkout");
  git(root, &["commit", "--quiet", "--all", "-m", "change"]);

  let again = scheduler
    .run_again(
      IdempotencyKey::generate(),
      project.task,
      first,
      Some(left_by(root, &failed)),
    )
    .await
    .expect("the task runs again");

  let next = wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
  // The removal runs off the runtime's threads, after the request is answered.
  let removed = tokio::time::timeout(Duration::from_secs(10), async {
    while first_index.exists() {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  })
  .await;
  assert!(removed.is_ok(), "the index went with the worktree that used it");
  assert_ne!(next.base(), failed.base());
  assert!(index_of(root, &next).is_dir());
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_reaches_codex_on_new_and_resumed_sessions_with_ripgrep_on_its_path() {
  let project = project(&indexed("Keep working."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  let run = killed_mid_run(root, project.task).await;
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  let ended = wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let given = CodeIndexUse::Given {
    revision: ended.base().clone(),
  };
  assert_eq!(uses(root, run), [given.clone(), given]);
  let index = index_of(root, &ended);
  let told = prompt_lines(root, run, "An index of this repository");
  assert_eq!(told.len(), 2, "new and resumed sessions are told: {told:?}");
  assert!(
    told
      .iter()
      .all(|line| line.contains(&index.to_string_lossy().into_owned()))
  );
  let skill = prompt_lines(root, run, "- codebase-map: ");
  assert_eq!(skill.len(), 1, "the new session lists the skill: {skill:?}");
  let document = skill[0]
    .rsplit_once(" (")
    .and_then(|(_, path)| path.strip_suffix(')'))
    .map(Path::new)
    .expect("the skill's path");
  let text = fs::read_to_string(document).expect("the placed skill");
  assert!(text.starts_with("---\nname: codebase-map\n"), "{text}");

  let db = ProjectDb::open(root).expect("open");
  let paths = reported(&db, run, "path: ");
  assert_eq!(paths.len(), 2);
  for path in paths {
    let first = std::env::split_paths(&path).next().expect("a PATH entry");
    assert!(first.starts_with(root.join(".orchester").join("tools")), "{path}");
    let copy = fs::read_to_string(first.join(ripgrep::PROGRAM)).expect("the copy");
    assert_eq!(copy, FAKE_RIPGREP);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_reaches_claude_code_as_a_folder_it_may_read() {
  let setup = Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..indexed("Append one line.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let _path = agent_path(root, true);
  let ended = run_to_the_end(root, project.task).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, ended.id(), "argv: ").pop().expect("argv");
  let index = index_of(root, &ended);
  assert!(argv.contains(&format!("--add-dir {}", index.display())), "{argv}");
  let skills = root.join(".orchester").join("skills");
  assert!(argv.contains(&format!("--add-dir {}", skills.display())), "{argv}");
  assert_eq!(prompt_lines(root, ended.id(), "An index of this repository").len(), 1);
  assert_eq!(prompt_lines(root, ended.id(), "- codebase-map: ").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_off_gives_agents_nothing_and_builds_nothing() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  let ended = run_to_the_end(root, project.task).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);

  assert_eq!(uses(root, ended.id()), [CodeIndexUse::Off]);
  assert!(!root.join(".orchester/index").exists());
  assert!(!root.join(".orchester/tools").exists());
  assert!(prompt_lines(root, ended.id(), "# Codebase index").is_empty());
  assert!(prompt_lines(root, ended.id(), "- codebase-map: ").is_empty());
  assert!(problems(root, ended.id()).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_ripgrep_the_agent_is_told_to_search_with_git_grep() {
  let project = project(&indexed("Say hello."));
  let root = project.repo.path();
  let _path = agent_path(root, false);
  let ended = run_to_the_end(root, project.task).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);

  let missing = CodeIndexUse::Unavailable {
    reason: CodeIndexUnavailable::RipgrepNotFound,
  };
  assert_eq!(uses(root, ended.id()), [missing]);
  assert_eq!(problems(root, ended.id()), [RunnerProblem::RipgrepNotFound]);
  assert_eq!(prompt_lines(root, ended.id(), "# Searching the repository").len(), 1);
  assert!(prompt_lines(root, ended.id(), "# Codebase index").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unavailable_codebase_index_is_reported_to_the_agent_and_the_run_goes_on() {
  let project = project(&indexed("Say hello."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  // A file where the index folder belongs: nothing can be built there.
  fs::write(root.join(".orchester/index"), "not a folder\n").expect("block the index folder");
  let ended = run_to_the_end(root, project.task).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);

  let problems = problems(root, ended.id());
  let [RunnerProblem::CodebaseIndexUnavailable { detail }] = &problems[..] else {
    panic!("one index problem: {problems:?}");
  };
  assert!(detail.contains("index"), "{detail}");
  assert!(matches!(
    &uses(root, ended.id())[..],
    [CodeIndexUse::Unavailable {
      reason: CodeIndexUnavailable::BuildFailed { .. }
    }]
  ));
  assert!(prompt_lines(root, ended.id(), "# Codebase index").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn tokens_before_the_first_edit_count_the_turns_until_the_agent_changed_a_file() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let ended = run_to_the_end(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  let invocation = db.invocations(ended.id()).expect("invocations").remove(0);
  let progress = db.edit_progress(invocation.id).expect("read").expect("recorded");
  // The fake's only turn changes README.md and reports its usage when it completes.
  assert_eq!(
    progress.before,
    TokenUsage {
      input: 12_166,
      cached_input: 2_432,
      output: 5,
    }
  );
  let edit = progress.first_edit.expect("the agent changed a file");
  assert_eq!(edit.path, "README.md");
  assert!(edit.at >= invocation.started_at);
}

#[tokio::test(flavor = "multi_thread")]
async fn tokens_before_the_first_edit_of_an_agent_that_changed_no_file_are_all_its_tokens() {
  let project = project(&Setup::new("Ask first."));
  let root = project.repo.path();
  let ended = run_to_the_end(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  let invocation = db.invocations(ended.id()).expect("invocations").remove(0);
  let progress = db.edit_progress(invocation.id).expect("read").expect("recorded");
  assert_eq!(progress.first_edit, None);
  assert_eq!(progress.before, invocation.usage);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_waiting_for_its_codebase_index_can_be_cancelled() {
  let project = project(&indexed("Say hello."));
  let root = project.repo.path();
  let _path = agent_path(root, true);
  // Another build of the revision holds its lock: the run waits for it.
  let base = git(root, &["rev-parse", "HEAD"]);
  let locks = root.join(".orchester").join("index").join(".locks");
  fs::create_dir_all(&locks).expect("create the locks folder");
  let held = fs::File::create(locks.join(&base)).expect("open the lock");
  held.lock().expect("lock");
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");

  tokio::time::timeout(Duration::from_secs(10), scheduler.cancel_run(run))
    .await
    .expect("the cancel is answered while the index is being built")
    .expect("cancelled");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;

  assert_eq!(ended.status(), &RunStatus::Cancelled);
  drop(held);
}
