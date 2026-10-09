//! Orchester's `list_skills` and `read_skill` end to end through the relay, with the fake CLIs: the skills placed
//! for the step execution (a pinned skill with a reference file) and the repository's skills the CLI itself loads:
//! Codex's `.agents/skills` while the project has its repository agent files on, none for Claude Code; a file that
//! leaves the skill is refused; the prompt names the skills without their paths and points at `read_skill`.
//! `read_file` reaches both kinds of skill as well. Needs `git`. Run
//! with `cargo test -p orchester-runner --features test-agent --test orchester_skill_tools orchester_skill_tools`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{InstructionScope, Skill, SkillFile, SkillFilePath, Slug},
  run::RunStatus,
  store::{BlobStore, ResourceStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, git, project, reported, scheduler_with, wait_for};

/// Publishes the project skill `run-checks` with the reference file `references/checks.md`.
fn publish_skill(root: &Path) {
  let mut db = ProjectDb::open(root).expect("open");
  let content = db.put_blob("text/markdown", b"cargo test").expect("blob");
  let file = SkillFile {
    path: SkillFilePath::new("references/checks.md").expect("path"),
    content,
    size: 10,
  };
  let skill = Skill::new(
    "Use run-checks when checks run.".to_owned(),
    "# run-checks\n\nRun them.".to_owned(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("valid skill");
  let id = db
    .create_resource(
      Slug::new("run-checks").expect("slug"),
      Name::new("run-checks").expect("name"),
      &skill,
      Timestamp::now(),
    )
    .expect("create")
    .id();
  db.publish_draft::<Skill>(id, Timestamp::now()).expect("publish");
}

/// Commits the repository skill `deploy` under `.agents/skills`, where Codex finds it, with a reference file.
fn commit_repository_skill(root: &Path) {
  let folder = root.join(".agents").join("skills").join("deploy");
  fs::create_dir_all(folder.join("references")).expect("skill folder");
  fs::write(
    folder.join("SKILL.md"),
    "---\nname: deploy\ndescription: Deploys.\n---\nDeploy.\n",
  )
  .expect("skill");
  fs::write(folder.join("references").join("steps.md"), "step one\n").expect("reference file");
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "repository skill"]);
}

/// A project of `provider` that pins `run-checks` and commits `deploy`, with its repository agent files `files`.
fn skills_project(provider: AgentProvider, permissions: AgentPermissions, files: RepositoryAgentFiles) -> TestProject {
  let project = project(&Setup {
    provider,
    permissions: Some(permissions),
    ..Setup::new("Use the skill tools.")
  });
  let root = project.repo.path();
  publish_skill(root);
  commit_repository_skill(root);
  ProjectDb::open(root)
    .expect("open")
    .save_repository_agent_files(files)
    .expect("save the setting");
  project
}

async fn finished(project: &TestProject) -> (RunId, ProjectDb) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id: RunId = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.pending_tool_approvals(id).expect("requests").is_empty(),
    "nothing asked"
  );
  (id, db)
}

fn codex_permissions() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  )
}

/// Each reported call starts as `starts` says, in order.
fn assert_calls(calls: &[String], starts: &[&str]) {
  assert_eq!(calls.len(), starts.len(), "{calls:#?}");
  for (call, start) in calls.iter().zip(starts) {
    assert!(call.starts_with(start), "{start}: {call}");
  }
}

/// What a `read_skill` call whose file leaves the skill is answered.
const LEAVES: &str = "read_skill error: invalid input for `read_skill`: a reference file path is";

/// The prompt names the pinned skill without its path, says to read skills with `tool`, and the Orchester tools
/// paragraph says so too.
fn assert_prompt(db: &ProjectDb, id: RunId, tool: &str) {
  let prompt = reported(db, id, "prompt| ").join("\n");
  assert!(
    prompt.contains("- run-checks: Use run-checks when checks run.\n"),
    "{prompt}"
  );
  assert!(!prompt.contains("run-checks/SKILL.md"), "{prompt}");
  assert!(
    prompt.contains(&format!("Read a skill with the {tool} tool by its name")),
    "{prompt}"
  );
  assert!(
    prompt.contains(&format!("Read your skills with {tool} by name, never by path")),
    "{prompt}"
  );
}

/// Codex lists its skills, reads the pinned one, its reference file and the repository's, and is refused files
/// that leave a skill; `read_file` reaches the placed skill by its absolute path and the repository's.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_skill_tools_serve_codex_pinned_and_repository_skills() {
  let project = skills_project(AgentProvider::CodexCli, codex_permissions(), RepositoryAgentFiles::On);
  let (id, db) = finished(&project).await;
  let listed = reported(&db, id, "orchester-tools: ").pop().expect("listed");
  assert!(
    listed.ends_with(",read_external_file,list_skills,read_skill"),
    "{listed}"
  );
  assert_calls(
    &reported(&db, id, "orchester-call: "),
    &[
      "list_skills ok: Read a skill with read_skill",
      "read_skill ok:      1\t---",
      "read_skill ok:      1\tcargo test",
      "read_skill ok:      1\tstep one",
      LEAVES,
      LEAVES,
      "read_file ok:      1\t---",
      "read_file ok:      1\t---",
    ],
  );
  let answers = reported(&db, id, "orchester-answer| ");
  for line in [
    "- run-checks (orchester): Use run-checks when checks run.",
    "  reference files: references/checks.md",
    "- deploy (repository): Deploys.",
    "  reference files: references/steps.md",
    "     8\tRun them.",
  ] {
    assert!(answers.contains(&line.to_owned()), "{line}: {answers:#?}");
  }
  assert_prompt(&db, id, "read_skill");
}

/// Claude Code calls both tools by their allow rules, through its permission prompt's relay. It gets the pinned
/// skill only: Claude Code loads no repository skill, whatever the setting says.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_skill_tools_serve_claude_code_by_their_allow_rules() {
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  let project = skills_project(AgentProvider::ClaudeCodeCli, permissions, RepositoryAgentFiles::On);
  let (id, db) = finished(&project).await;
  let listed = reported(&db, id, "orchester-tools: ").pop().expect("listed");
  let names: Vec<&str> = listed.split(',').collect();
  assert!(
    names.contains(&"list_skills") && names.contains(&"read_skill"),
    "{listed}"
  );
  let decisions = reported(&db, id, "orchester-permission: ");
  assert!(
    decisions.iter().all(|line| line.ends_with("allow-by-rule")),
    "{decisions:#?}"
  );
  let calls = reported(&db, id, "orchester-call: ");
  assert_calls(
    &calls,
    &[
      "list_skills ok",
      "read_skill ok",
      "read_skill ok",
      "read_skill error",
      "read_skill error",
      "read_skill error",
      "read_file ok",
      "read_file ok",
    ],
  );
  assert!(
    calls[0].ends_with("|   reference files: references/checks.md"),
    "{calls:#?}"
  );
  assert!(calls[1].ends_with("|      8\tRun them."), "{calls:#?}");
  assert!(
    calls[2].ends_with(":      1\tcargo test |      1\tcargo test"),
    "{calls:#?}"
  );
  assert!(calls[3].contains("this step has no skill named `deploy`"), "{calls:#?}");
  assert!(calls[4].contains("a reference file path is"), "{calls:#?}");
  assert_prompt(&db, id, "mcp__Orchester__read_skill");
}

/// With the project's repository agent files off, the repository's skill is not offered: only the pinned one is
/// listed and read. The repository's `SKILL.md` stays an ordinary file of the worktree.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_skill_tools_leave_repository_skills_out_while_agent_files_are_off() {
  let project = skills_project(AgentProvider::CodexCli, codex_permissions(), RepositoryAgentFiles::Off);
  let (id, db) = finished(&project).await;
  assert_calls(
    &reported(&db, id, "orchester-call: "),
    &[
      "list_skills ok",
      "read_skill ok:      1\t---",
      "read_skill ok:      1\tcargo test",
      "read_skill error: this step has no skill named `deploy`",
      LEAVES,
      LEAVES,
      "read_file ok",
      "read_file ok",
    ],
  );
  let answers = reported(&db, id, "orchester-answer| ");
  assert!(
    answers.contains(&"- run-checks (orchester): Use run-checks when checks run.".to_owned()),
    "{answers:#?}"
  );
  assert!(
    !answers.iter().any(|line| line.contains("(repository)")),
    "{answers:#?}"
  );
  assert_prompt(&db, id, "read_skill");
}
