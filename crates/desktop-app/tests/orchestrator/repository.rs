//! The chat's `init_repository` (ORC-72): its card shows the branch, what the first commit holds, the whole
//! `.gitignore` and the files that look secret, and only a person applies it: automation's click is refused
//! (`person_only`), and the person's makes the first commit. Kontur's repository has no commit here, as the person's
//! had on 2026-10-21; the window runs git with git's settings in a file of the test's own (`GIT_CONFIG_GLOBAL`, no
//! system settings, no author from the environment), the author the repository's own settings name.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  orchestrator::ActionOutcome,
};
use orchester_exec::{ProcessRunner, programs::Programs};

use super::{
  harness::{Orchestrator, Setup, git},
  proposal::{outcomes, proposal_card},
};

/// git with the settings file `global` only.
struct Isolated {
  global: PathBuf,
}

impl CommandRunner for Isolated {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut isolated = command
      .clone()
      .with_env("GIT_CONFIG_GLOBAL", &self.global.to_string_lossy())
      .with_env("GIT_CONFIG_NOSYSTEM", "1");
    for variable in [
      "GIT_AUTHOR_NAME",
      "GIT_AUTHOR_EMAIL",
      "GIT_COMMITTER_NAME",
      "GIT_COMMITTER_EMAIL",
      "EMAIL",
    ] {
      isolated = isolated.without_env(variable);
    }
    ProcessRunner::new(Duration::from_secs(60)).run(&isolated)
  }
}

/// Kontur, its repository without a commit and a `.env` beside its readme, the window's git isolated; and the folder
/// that holds git's settings.
fn without_a_commit() -> (Orchestrator, tempfile::TempDir) {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orchestrator-tests");
  fs::create_dir_all(&scratch).expect("scratch directory");
  let settings = tempfile::tempdir_in(scratch).expect("a folder for git's settings");
  let global = settings.path().join("gitconfig");
  fs::write(&global, "[user]\n\tuseConfigOnly = true\n").expect("git's settings");
  let programs = Programs::with_runner(Arc::new(Isolated { global }));
  let screen = Orchestrator::with_git(Setup::Ready, Some(programs));
  git(&screen.root, &["update-ref", "-d", "HEAD"]);
  fs::write(screen.root.join(".env"), "TOKEN=1\n").expect("a .env");
  (screen, settings)
}

#[test]
fn orchestrator_repository_card_is_applied_only_by_a_person() {
  let (mut screen, _settings) = without_a_commit();
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call init_repository {"project":"Kontur","first_commit":"files","gitignore":"target/"}"#);
  screen.until("the proposal", |screen| {
    outcomes(screen).first() == Some(&ActionOutcome::Waits)
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(&screen);
  screen.until("the card", |screen| screen.exists(&format!("{card}-start")));
  assert_eq!(
    screen.text(&format!("{card}-title")),
    "Make the first commit of Kontur's repository?"
  );
  let facts = screen.text(&format!("{card}-facts"));
  for shown in ["main", "Initial commit · 3 files", ".orchester stays out of git"] {
    assert!(facts.contains(shown), "{shown}: {facts}");
  }
  assert_eq!(screen.text(&format!("{card}-gitignore")).trim(), "target/");
  assert!(
    screen.text(&format!("{card}-sensitive")).contains(".env"),
    "the person decides on the .env"
  );
  assert!(
    !screen.root.join(".gitignore").exists(),
    "nothing is written on the agent's word"
  );

  // Pressed until the card takes presses: its button waits while the card reads.
  let (start, person_only) = (format!("{card}-start"), format!("{card}-person-only"));
  screen.until("the refusal", |screen| {
    with_synthetic_input(|| screen.click(&start));
    screen.exists(&person_only)
  });
  assert_eq!(
    outcomes(&screen)[0],
    ActionOutcome::Waits,
    "automation's click applies nothing"
  );
  assert!(!screen.root.join(".gitignore").exists());

  screen.click(&start);
  screen.until("the commit", |screen| outcomes(screen)[0] == ActionOutcome::Applied);
  let committed = std::process::Command::new("git")
    .arg("-C")
    .arg(&screen.root)
    .args(["ls-tree", "-r", "--name-only", "HEAD"])
    .output()
    .expect("git runs");
  let committed = String::from_utf8_lossy(&committed.stdout).into_owned();
  let mut files: Vec<&str> = committed.lines().collect();
  files.sort_unstable();
  assert_eq!(
    files,
    [".env", ".gitignore", "README.md"],
    "the person's Apply committed them"
  );
  screen.until("the applied card", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Made the first commit of Kontur's repository")
  });
}
