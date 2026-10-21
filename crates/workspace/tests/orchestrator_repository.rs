//! The Orchestrator chat's `init_repository` (ORC-72): what its card proposes for a project's folder and what applying
//! it makes, through the setup page's own functions, with real git in folders of the tests' own under the worktree's
//! `.tmp/`. git's settings are a file of each test's own (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`, no author from
//! the environment), so the person's git settings are never read or written, and git looks for no repository above
//! each test's folder (`GIT_CEILING_DIRECTORIES`).

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  orchestrator::change::{Change, FirstCommitOf, Gitignore},
};
use orchester_exec::ProcessRunner;
use orchester_workspace::{
  app_mcp::ToolError,
  orchestrator::repository::{apply, propose},
};
use serde_json::{Value, json};
use tempfile::TempDir;

const AUTHOR: &str = "[user]\n\tname = Test Author\n\temail = author@example.com\n";

/// git with the settings file `global` only, looking for no repository above `ceiling`; with `program`, a git that is
/// not installed.
struct Isolated {
  global: PathBuf,
  ceiling: PathBuf,
  program: Option<PathBuf>,
}

impl CommandRunner for Isolated {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut isolated = match &self.program {
      Some(program) => Command::new(program, command.args().to_vec()),
      None => command.clone(),
    };
    if let Some(dir) = command.current_dir() {
      isolated = isolated.with_current_dir(dir);
    }
    isolated = isolated
      .with_env("GIT_CONFIG_GLOBAL", &self.global.to_string_lossy())
      .with_env("GIT_CONFIG_NOSYSTEM", "1")
      .with_env("GIT_CEILING_DIRECTORIES", &self.ceiling.to_string_lossy());
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

/// A project's folder of the test's own, holding Orchester's `.orchester/` as every project's does, with git's
/// settings beside it.
struct Scratch {
  dir: TempDir,
  git: Isolated,
}

impl Scratch {
  fn new(settings: &str) -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/git-init/chat-tests");
    fs::create_dir_all(&root).expect("a scratch folder");
    let dir = tempfile::tempdir_in(root).expect("a folder of the test's own");
    let global = dir.path().join("gitconfig");
    fs::write(&global, format!("[user]\n\tuseConfigOnly = true\n{settings}")).expect("git's settings");
    let scratch = Self {
      git: Isolated {
        global,
        ceiling: dir.path().to_path_buf(),
        program: None,
      },
      dir,
    };
    scratch.file(".orchester/orchester.db", "the project's data");
    scratch
  }

  fn folder(&self) -> PathBuf {
    self.dir.path().join("kontur")
  }

  fn file(&self, name: &str, text: &str) {
    let path = self.folder().join(name);
    fs::create_dir_all(path.parent().expect("a file in the folder")).expect("its folder");
    fs::write(path, text).expect("a file");
  }

  fn ask(&self, args: &[&str]) -> String {
    let output = self
      .git
      .run(&Command::new(Path::new("git"), args.iter().copied()).with_current_dir(&self.folder()))
      .expect("git runs");
    assert!(output.success(), "git {args:?}: {}", output.stderr);
    output.stdout.trim().to_owned()
  }

  fn committed(&self) -> Vec<String> {
    let mut files: Vec<String> = self
      .ask(&["ls-tree", "-r", "--name-only", "HEAD"])
      .lines()
      .map(str::to_owned)
      .collect();
    files.sort();
    files
  }

  /// The chat's `init_repository` with `arguments`, in this folder.
  fn propose(&self, arguments: Value) -> Result<Change, ToolError> {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    propose(&self.git, (&self.folder(), "Kontur"), &arguments).map(|proposed| {
      assert_eq!(proposed.subject, "Kontur", "the card names the project");
      proposed.change
    })
  }

  fn apply(&self, change: &Change) -> Result<Value, ToolError> {
    apply(&self.git, &self.folder(), change)
  }
}

fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal, not {other:?}"),
  }
}

/// The English words of the workspace's message `key`.
fn english(key: &str) -> String {
  let words: Value = serde_json::from_str(include_str!("../assets/locales/en.json")).expect("the locale file");
  key
    .split('.')
    .fold(&words, |at, part| &at[part])
    .as_str()
    .unwrap_or_else(|| panic!("{key} has words"))
    .to_owned()
}

#[test]
fn init_repository_proposes_and_makes_an_empty_first_commit_in_an_empty_folder() {
  let scratch = Scratch::new(AUTHOR);
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "empty" }))
    .expect("proposed");
  assert_eq!(
    change,
    Change::InitRepository {
      branch: "main".to_owned(),
      first: FirstCommitOf::Empty,
      files: 0,
      exists: false,
      gitignore: None,
      sensitive: Vec::new(),
    }
  );
  assert!(!scratch.folder().join(".git").exists(), "proposing makes nothing");
  let made = scratch.apply(&change).expect("applied");
  assert_eq!(made["repository"]["branch"], "main", "{made}");
  assert_eq!(scratch.ask(&["rev-list", "--count", "HEAD"]), "1");
  assert_eq!(scratch.ask(&["log", "-1", "--format=%s"]), "Initial commit");
  assert!(scratch.committed().is_empty(), "an empty first commit holds nothing");
  assert!(scratch.ask(&["remote"]).is_empty(), "no remote is added");
  let exclude = fs::read_to_string(scratch.folder().join(".git/info/exclude")).expect("the exclude file");
  assert!(exclude.lines().any(|line| line == "/.orchester/"), "{exclude}");
}

#[test]
fn init_repository_commits_the_files_and_leaves_out_what_the_gitignore_lists() {
  let scratch = Scratch::new(AUTHOR);
  scratch.file("README.md", "Kontur\n");
  scratch.file("src/main.rs", "fn main() {}\n");
  scratch.file("target/debug/kontur.exe", "binary\n");
  scratch.file(".env", "TOKEN=1\n");
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files", "gitignore": "target/" }))
    .expect("proposed");
  let Change::InitRepository {
    files,
    gitignore,
    sensitive,
    ..
  } = &change
  else {
    panic!("a first commit: {change:?}");
  };
  assert_eq!(*files, 4, "the .gitignore, .env, README.md and src/main.rs");
  assert_eq!(
    gitignore,
    &Some(Gitignore::Writes {
      text: "target/\n".to_owned()
    })
  );
  assert_eq!(sensitive, &[".env".to_owned()], "the person decides on the .env");
  assert!(
    !scratch.folder().join(".gitignore").exists() && !scratch.folder().join(".git").exists(),
    "proposing writes nothing"
  );
  let made = scratch.apply(&change).expect("applied");
  assert_eq!(made["repository"]["files"], 4, "{made}");
  assert_eq!(
    scratch.committed(),
    [".env", ".gitignore", "README.md", "src/main.rs"],
    "target/ and .orchester/ stay out"
  );
  assert_eq!(
    fs::read_to_string(scratch.folder().join(".gitignore")).expect("written"),
    "target/\n"
  );
}

#[test]
fn init_repository_keeps_the_folders_own_gitignore_and_never_writes_over_it() {
  let scratch = Scratch::new(AUTHOR);
  scratch.file(".gitignore", "*.log\n");
  scratch.file("README.md", "Kontur\n");
  scratch.file("run.log", "noise\n");
  let refused = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files", "gitignore": "build/\n" }))
    .expect_err("a .gitignore is never written over");
  assert_eq!(code(&refused), "gitignore_exists");
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect("proposed");
  assert!(matches!(
    &change,
    Change::InitRepository { files: 2, gitignore: Some(Gitignore::Kept { text }), .. } if text == "*.log\n"
  ));
}

#[test]
fn init_repository_refuses_a_repository_with_a_commit_and_leaves_it_as_it_was() {
  let scratch = Scratch::new(AUTHOR);
  scratch.file("README.md", "Kontur\n");
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect("proposed");
  // Meanwhile the person made the repository and its first commit in a terminal.
  scratch.ask(&["init", "--quiet", "--initial-branch=develop"]);
  scratch.ask(&["add", "README.md"]);
  scratch.ask(&["commit", "--quiet", "-m", "Theirs"]);
  let head = scratch.ask(&["rev-parse", "HEAD"]);
  scratch.file("notes.txt", "uncommitted\n");
  let refused = scratch.apply(&change).expect_err("Apply checks again");
  assert_eq!(code(&refused), "has_commits");
  assert_eq!(scratch.ask(&["rev-parse", "HEAD"]), head, "its history is untouched");
  assert!(
    scratch.ask(&["diff", "--cached", "--name-only"]).is_empty(),
    "nothing staged"
  );
  assert!(
    !fs::read_to_string(scratch.folder().join(".git/info/exclude"))
      .unwrap_or_default()
      .contains("/.orchester/"),
    "nothing written in the repository"
  );
  let again = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect_err("nothing to propose");
  assert_eq!(code(&again), "has_commits");
}

#[test]
fn init_repository_gives_a_repository_without_a_commit_its_first_commit() {
  let scratch = Scratch::new(AUTHOR);
  scratch.file("README.md", "Kontur\n");
  scratch.ask(&["init", "--quiet", "--initial-branch=trunk"]);
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect("proposed");
  assert!(matches!(
    &change,
    Change::InitRepository { branch, exists: true, files: 1, .. } if branch == "trunk"
  ));
  scratch.apply(&change).expect("applied");
  assert_eq!(scratch.committed(), ["README.md"]);
  assert_eq!(scratch.ask(&["symbolic-ref", "--short", "HEAD"]), "trunk");
}

#[test]
fn init_repository_without_an_author_is_refused_with_the_git_config_commands() {
  let scratch = Scratch::new("");
  scratch.file("README.md", "Kontur\n");
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect("proposed");
  let refused = scratch.apply(&change).expect_err("git does not know the author");
  let ToolError::Refused { code, reasons } = &refused else {
    panic!("a refusal: {refused:?}");
  };
  assert_eq!(code, "author_unknown");
  assert_eq!(reasons[0].key, "orchestrator.repository.authorUnknown");
  assert!(
    reasons[0]
      .args
      .iter()
      .any(|(name, said)| *name == "said" && !said.is_empty()),
    "git's own words: {reasons:?}"
  );
  let words = english(reasons[0].key);
  for hint in [
    "git config --global user.name \"Your Name\"",
    "git config --global user.email \"you@example.com\"",
  ] {
    assert!(words.contains(hint), "{words}");
  }
}

#[test]
fn init_repository_says_git_is_missing() {
  let mut scratch = Scratch::new(AUTHOR);
  scratch.git.program = Some(scratch.dir.path().join("no-git").join("git"));
  let refused = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "empty" }))
    .expect_err("no git");
  assert_eq!(code(&refused), "git_not_found");
}

#[test]
fn init_repository_refuses_files_that_look_secret_and_appeared_since_it_was_proposed() {
  let scratch = Scratch::new(AUTHOR);
  scratch.file("README.md", "Kontur\n");
  let change = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect("proposed");
  scratch.file("deploy/id_ed25519", "a private key\n");
  let refused = scratch.apply(&change).expect_err("the person was not shown it");
  assert_eq!(code(&refused), "changed_since_proposed");
  assert!(!scratch.folder().join(".git").exists(), "nothing was made");
}

#[test]
fn init_repository_refuses_a_first_commit_of_files_where_there_are_none() {
  let scratch = Scratch::new(AUTHOR);
  let refused = scratch
    .propose(json!({ "project": "Kontur", "first_commit": "files" }))
    .expect_err("only .orchester/ is there");
  assert_eq!(code(&refused), "nothing_to_commit");
}
