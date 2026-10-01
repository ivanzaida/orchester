//! The repository's own Codex settings (`.codex/`), end to end against `orchester-fake-codex`: the
//! worktree and its repository are marked untrusted on every start, so Codex leaves the folder's
//! configuration, hooks, rules and MCP servers out and the activity says the branch has them; a project
//! layer Codex loaded all the same refuses the step, and the folder's skills are disabled while the
//! project's agent files are off. Needs `git`. Run with `cargo test -p orchester-runner --features
//! test-agent --test codex_repository_settings codex_repository_settings`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId},
  run::RunStatus,
  store::{ActivityStore, SettingsStore},
};
use orchester_store::ProjectDb;

use self::support::{Setup, git, project, reported, scheduler, threads, wait_for};

/// Commits `files` (path under the repository, content), so every worktree of a run has them.
fn commit(root: &Path, files: &[(&str, &str)]) {
  for (path, content) in files {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
    fs::write(path, content).expect("file");
  }
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "repository codex settings"]);
}

/// The repository's Codex settings a project might commit: an allow rule, a server and a hook.
const SETTINGS: [(&str, &str); 3] = [
  (
    ".codex/rules/allow.rules",
    "prefix_rule(pattern=[\"cmake\"], decision=\"allow\")\n",
  ),
  (
    ".codex/config.toml",
    "[mcp_servers.repository]\nurl = \"http://127.0.0.1:9/mcp\"\n",
  ),
  (".codex/hooks.json", "{\"hooks\":{}}\n"),
];

async fn run_until(root: &Path, task: orchester_core::id::TaskId, done: fn(&RunStatus) -> bool) -> RunId {
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| done(run.status())).await;
  run
}

fn problems(db: &ProjectDb, run: RunId) -> Vec<RunnerProblem> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect()
}

/// The worktree as the runner named it to Codex.
fn worktree(db: &ProjectDb, run: RunId) -> String {
  threads(db, run)[0]["cwd"]
    .as_str()
    .expect("the thread's working folder")
    .to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_repository_settings_are_left_out_and_the_activity_says_so() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  commit(root, &SETTINGS);
  let run = run_until(root, project.task, |status| status == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, run, "argv: ").pop().expect("argv reported");
  assert!(argv.contains(" -c projects={"), "{argv}");
  assert!(argv.contains("={trust_level=\"untrusted\"}"), "{argv}");
  // Named in the worktree, not by its absolute path.
  assert_eq!(
    problems(&db, run),
    [RunnerProblem::CodexSettingsIgnored {
      folder: format!(".codex{}", std::path::MAIN_SEPARATOR)
    }]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_repository_settings_without_a_folder_leave_no_note() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let run = run_until(root, project.task, |status| status == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(problems(&db, run), []);
}

/// Should Codex load the folder all the same (here: the fake ignores the untrusted mark), the step
/// stops before its thread starts and says why.
#[tokio::test(flavor = "multi_thread")]
async fn codex_repository_settings_codex_loaded_anyway_refuse_the_step() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  commit(root, &[SETTINGS[1], (".codex/fake-loads-anyway", "")]);
  let run = run_until(root, project.task, |status| matches!(status, RunStatus::Failed(_))).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(threads(&db, run).is_empty(), "no thread starts");
  let failures: Vec<String> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::TurnFailed { message } => Some(message),
      _ => None,
    })
    .collect();
  assert!(
    failures
      .iter()
      .any(|message| message.contains("Codex loaded the repository's own Codex settings")),
    "{failures:?}"
  );
}

/// Codex loads the skills of `.codex/skills` even for an untrusted repository, so they are disabled
/// with the other repository skills while the project's agent files are off.
#[tokio::test(flavor = "multi_thread")]
async fn codex_repository_settings_skills_follow_repository_agent_files() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  commit(
    root,
    &[(
      ".codex/skills/deploy/SKILL.md",
      "---\nname: deploy\ndescription: Deploys.\n---\nDeploy.\n",
    )],
  );
  ProjectDb::open(root)
    .expect("open")
    .save_repository_agent_files(RepositoryAgentFiles::Off)
    .expect("save the setting");
  let run = run_until(root, project.task, |status| status == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, run, "argv: ").pop().expect("argv reported");
  let skill = Path::new(&worktree(&db, run))
    .join(".codex")
    .join("skills")
    .join("deploy")
    .join("SKILL.md");
  let disabled = format!(
    "skills.config=[{{path={},enabled=false}}]",
    serde_json::to_string(&skill.to_string_lossy()).expect("quoted")
  );
  assert!(argv.contains(&disabled), "{argv}\n{disabled}");
}
