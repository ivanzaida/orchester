//! Skills and repository agent files at an agent's start (ORC-15), end to end against
//! `orchester-fake-codex` and `orchester-fake-claude`: the project's setting reaches the CLI on new
//! and resumed sessions, and the skills that reach the agent are placed outside the worktree and
//! listed in its prompt. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test agent_files`.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  name::Name,
  resource::{InstructionScope, Skill, SkillFile, SkillFilePath, Slug},
  run::RunStatus,
  store::{ActivityStore, BlobStore, ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use self::support::{Setup, git, killed_mid_run, project, reported, scheduler, threads, wait_for};

/// Commits a repository skill for Codex, so every worktree of a run has it.
fn commit_repository_skill(root: &Path) {
  let folder = root.join(".agents").join("skills").join("deploy");
  fs::create_dir_all(&folder).expect("skill folder");
  fs::write(
    folder.join("SKILL.md"),
    "---\nname: deploy\ndescription: Deploys.\n---\nDeploy.\n",
  )
  .expect("skill");
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "repository skill"]);
}

fn set_agent_files(root: &Path, setting: RepositoryAgentFiles) {
  ProjectDb::open(root)
    .expect("open")
    .save_repository_agent_files(setting)
    .expect("save the setting");
}

/// Publishes a skill of `scope` with one reference file.
fn publish_skill(root: &Path, slug: &str, scope: InstructionScope) {
  let mut db = ProjectDb::open(root).expect("open");
  let content = db.put_blob("text/markdown", b"cargo test").expect("blob");
  let file = SkillFile {
    path: SkillFilePath::new("references/checks.md").expect("path"),
    content,
    size: 10,
  };
  let skill = Skill::new(
    format!("Use {slug} when checks run."),
    format!("# {slug}\n\nRun them."),
    vec![file],
    scope,
  )
  .expect("valid skill");
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      Name::new(slug).expect("name"),
      &skill,
      Timestamp::now(),
    )
    .expect("create")
    .id();
  db.publish_draft::<Skill>(id, Timestamp::now()).expect("publish");
}

async fn run_to_success(root: &Path, task: orchester_core::id::TaskId) -> RunId {
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  run
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_files_codex_loads_them_while_the_setting_is_on() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  commit_repository_skill(root);
  let run = run_to_success(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, run, "argv: ").pop().expect("argv reported");
  assert!(!argv.contains("project_doc_max_bytes"), "{argv}");
  assert!(!argv.contains("skills.config"), "{argv}");
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_files_off_reach_codex_on_new_and_resumed_sessions() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  commit_repository_skill(root);
  set_agent_files(root, RepositoryAgentFiles::Off);
  let run = killed_mid_run(root, project.task).await;
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, run, "argv: ");
  assert_eq!(argv.len(), 2, "{argv:?}");
  let threads = threads(&db, run);
  // The worktree as the runner names it, which the resumed session works in too.
  let worktree = threads[0]["cwd"]
    .as_str()
    .map(Path::new)
    .expect("a new session is started in its worktree");
  let skill = worktree.join(".agents").join("skills").join("deploy").join("SKILL.md");
  let disabled = format!(
    "skills.config=[{{path={},enabled=false}}]",
    serde_json::to_string(&skill.to_string_lossy()).expect("quoted")
  );
  for ((argv, thread), resumed) in argv.iter().zip(&threads).zip([false, true]) {
    assert_eq!(thread.get("threadId").is_some(), resumed, "{thread}");
    assert!(argv.contains("-c project_doc_max_bytes=0"), "{argv}");
    assert!(argv.contains(&disabled), "{argv}\n{disabled}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_files_skills_are_placed_outside_the_worktree_and_listed_for_codex() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  publish_skill(root, "run-checks", InstructionScope::Project);
  publish_skill(
    root,
    "review-rules",
    InstructionScope::AgentRole {
      role: Name::new("Reviewer").expect("role"),
    },
  );
  let run = run_to_success(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  let execution = db.invocations(run).expect("invocations")[0].execution;
  let folder = root
    .join(".orchester")
    .join("skills")
    .join(execution.to_string())
    .join("run-checks");
  let document = fs::read_to_string(folder.join("SKILL.md")).expect("SKILL.md placed");
  assert!(document.starts_with("---\nname: \"run-checks\"\ndescription: \"Use run-checks when checks run.\"\n---\n"));
  assert_eq!(
    fs::read(folder.join("references").join("checks.md")).expect("reference file placed"),
    b"cargo test"
  );
  assert!(
    !root
      .join(".orchester")
      .join("skills")
      .join(execution.to_string())
      .join("review-rules")
      .exists()
  );
  let prompt = reported(&db, run, "prompt| ").join("\n");
  assert!(prompt.contains("# Skills"), "{prompt}");
  // Codex reads it with Orchester's read_skill by name: the prompt gives no path in the placed folder.
  assert!(
    prompt.contains("- run-checks: Use run-checks when checks run.\n"),
    "{prompt}"
  );
  assert!(
    prompt.contains("Read a skill with the read_skill tool by its name"),
    "{prompt}"
  );
  assert!(!prompt.contains(&folder.display().to_string()), "{prompt}");
  assert!(!prompt.contains("review-rules"), "{prompt}");
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_files_claude_reads_the_skills_folder_and_never_loads_project_settings() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Append one line.")
  });
  let root = project.repo.path();
  publish_skill(root, "run-checks", InstructionScope::Project);
  set_agent_files(root, RepositoryAgentFiles::Off);
  let run = run_to_success(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  let execution = db.invocations(run).expect("invocations")[0].execution;
  let folder = root.join(".orchester").join("skills").join(execution.to_string());
  let argv = reported(&db, run, "argv: ").pop().expect("argv reported");
  assert!(argv.contains("--setting-sources  --disable-slash-commands"), "{argv}");
  assert!(argv.contains(&format!("--add-dir {}", folder.display())), "{argv}");
  assert!(!argv.contains("project_doc_max_bytes"), "{argv}");
  let prompt = reported(&db, run, "prompt| ").join("\n");
  assert!(
    prompt.contains("Read a skill with the mcp__Orchester__read_skill tool by its name"),
    "{prompt}"
  );
  assert!(
    prompt.contains("- run-checks: Use run-checks when checks run.\n"),
    "{prompt}"
  );
}
