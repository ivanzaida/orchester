//! Making a folder a git repository in place (`repository::init`), with real git in folders of the tests' own under
//! the worktree's `.tmp/`. git's settings are a file of each test's own (`GIT_CONFIG_GLOBAL`, no system settings,
//! no identity from the environment), so the person's git settings are never read or written.

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};
use orchester_exec::ProcessRunner;
use orchester_workspace::repository::init::{
  FALLBACK_BRANCH, FirstCommit, InitError, LocalRepository, SurveyError, create_repository, init_repository,
  make_first_commit, read_local, survey,
};
use tempfile::TempDir;

/// Variables through which git could find an author without its settings.
const IDENTITY_VARIABLES: [&str; 5] = [
  "GIT_AUTHOR_NAME",
  "GIT_AUTHOR_EMAIL",
  "GIT_COMMITTER_NAME",
  "GIT_COMMITTER_EMAIL",
  "EMAIL",
];

/// git with the settings file `global` only.
struct Isolated {
  global: PathBuf,
  /// Runs this program in place of git: one that does not exist stands for git not installed.
  program: Option<PathBuf>,
}

impl CommandRunner for Isolated {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut isolated = match &self.program {
      Some(program) => {
        let mut moved = Command::new(program, command.args().to_vec());
        if let Some(dir) = command.current_dir() {
          moved = moved.with_current_dir(dir);
        }
        moved
      }
      None => command.clone(),
    }
    .with_env("GIT_CONFIG_GLOBAL", &self.global.to_string_lossy())
    .with_env("GIT_CONFIG_NOSYSTEM", "1");
    for variable in IDENTITY_VARIABLES {
      isolated = isolated.without_env(variable);
    }
    ProcessRunner::new(Duration::from_secs(60)).run(&isolated)
  }
}

/// A folder of the test's own with git's settings beside it.
struct Scratch {
  dir: TempDir,
  git: Isolated,
}

impl Scratch {
  /// git's settings say who the author is and nothing else.
  fn new() -> Self {
    Self::with_settings("[user]\n\tname = Test Author\n\temail = author@example.com\n")
  }

  /// git's settings are `settings`; `useConfigOnly` keeps git from guessing an author from the computer.
  fn with_settings(settings: &str) -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/git-init/tests");
    fs::create_dir_all(&root).expect("a scratch folder");
    let dir = tempfile::tempdir_in(root).expect("a folder of the test's own");
    let global = dir.path().join("gitconfig");
    write_settings(&global, settings);
    fs::create_dir_all(dir.path().join("folder")).expect("the folder");
    Self {
      git: Isolated { global, program: None },
      dir,
    }
  }

  fn folder(&self) -> PathBuf {
    self.dir.path().join("folder")
  }

  fn file(&self, name: &str, text: &str) {
    let path = self.folder().join(name);
    fs::create_dir_all(path.parent().expect("a file in the folder")).expect("its folder");
    fs::write(path, text).expect("a file");
  }

  /// What git answers in the folder, trimmed; panics when it fails.
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
}

fn write_settings(path: &Path, settings: &str) {
  fs::write(path, format!("[user]\n\tuseConfigOnly = true\n{settings}")).expect("git's settings");
}

#[test]
fn git_init_an_empty_folder_makes_an_empty_first_commit_on_the_default_branch() {
  let scratch = Scratch::new();
  let surveyed = survey(&scratch.git, &scratch.folder()).expect("the folder is surveyed");
  assert_eq!(surveyed.files, 0);
  assert!(!surveyed.gitignore);
  assert_eq!(surveyed.branch.name(), FALLBACK_BRANCH);
  assert!(!surveyed.branch.is_configured());
  assert_eq!(
    read_local(&scratch.git, &scratch.folder()),
    LocalRepository::NotRepository
  );
  let made = init_repository(&scratch.git, &scratch.folder(), FirstCommit::Empty).expect("the repository is made");
  assert_eq!((made.branch.as_str(), made.files), (FALLBACK_BRANCH, 0));
  assert_eq!(scratch.ask(&["rev-list", "--count", "HEAD"]), "1");
  assert_eq!(
    scratch.ask(&["log", "-1", "--format=%s|%an"]),
    "Initial commit|Test Author"
  );
  assert!(scratch.committed().is_empty(), "an empty first commit holds nothing");
  assert_eq!(
    read_local(&scratch.git, &scratch.folder()),
    LocalRepository::Commits {
      branch: Some(FALLBACK_BRANCH.to_owned()),
      count: Some(1)
    }
  );
  assert!(scratch.ask(&["remote"]).is_empty(), "no remote is added");
}

#[test]
fn git_init_commits_the_folders_files_and_leaves_out_what_gitignore_lists() {
  let scratch = Scratch::new();
  scratch.file(".gitignore", "*.log\nbuild/\n");
  scratch.file("README.md", "Kontur\n");
  scratch.file("src/main.rs", "fn main() {}\n");
  scratch.file("logs/run.log", "noise\n");
  scratch.file("build/out.bin", "binary\n");
  let surveyed = survey(&scratch.git, &scratch.folder()).expect("the folder is surveyed");
  assert_eq!(surveyed.files, 3, "what git add -A would add");
  assert!(surveyed.gitignore);
  assert!(
    !scratch.folder().join(".git").exists(),
    "counting makes nothing in the folder"
  );
  let made = init_repository(&scratch.git, &scratch.folder(), FirstCommit::Files).expect("the repository is made");
  assert_eq!(made.files, 3);
  assert_eq!(scratch.committed(), [".gitignore", "README.md", "src/main.rs"]);
}

#[test]
fn git_init_an_empty_first_commit_leaves_the_folders_files_uncommitted() {
  let scratch = Scratch::new();
  scratch.file("README.md", "Kontur\n");
  init_repository(&scratch.git, &scratch.folder(), FirstCommit::Empty).expect("the repository is made");
  assert!(scratch.committed().is_empty());
  assert_eq!(scratch.ask(&["status", "--porcelain"]), "?? README.md");
}

#[test]
fn git_init_honours_init_default_branch() {
  let scratch = Scratch::with_settings(
    "[user]\n\tname = Test Author\n\temail = author@example.com\n[init]\n\tdefaultBranch = trunk\n",
  );
  let surveyed = survey(&scratch.git, &scratch.folder()).expect("the folder is surveyed");
  assert_eq!(surveyed.branch.name(), "trunk");
  assert!(surveyed.branch.is_configured());
  let made = init_repository(&scratch.git, &scratch.folder(), FirstCommit::Empty).expect("the repository is made");
  assert_eq!(made.branch, "trunk");
  assert_eq!(scratch.ask(&["symbolic-ref", "--short", "HEAD"]), "trunk");
}

#[test]
fn git_init_refuses_an_existing_repository_and_leaves_it_as_it_was() {
  let scratch = Scratch::new();
  scratch.file("README.md", "Kontur\n");
  scratch.ask(&["init", "--quiet", "--initial-branch=develop"]);
  scratch.ask(&["add", "-A"]);
  scratch.ask(&["commit", "--quiet", "-m", "Theirs"]);
  let head = scratch.ask(&["rev-parse", "HEAD"]);
  scratch.file("notes.txt", "uncommitted\n");
  for first in [FirstCommit::Files, FirstCommit::Empty] {
    assert_eq!(
      init_repository(&scratch.git, &scratch.folder(), first),
      Err(InitError::AlreadyRepository)
    );
  }
  assert_eq!(
    make_first_commit(&scratch.git, &scratch.folder(), FirstCommit::Files),
    Err(InitError::HasCommits)
  );
  assert_eq!(scratch.ask(&["rev-parse", "HEAD"]), head);
  assert_eq!(scratch.ask(&["symbolic-ref", "--short", "HEAD"]), "develop");
  assert_eq!(
    scratch.ask(&["status", "--porcelain"]),
    "?? notes.txt",
    "nothing was staged"
  );
}

#[test]
fn git_init_without_an_author_says_so_and_try_again_makes_only_the_commit() {
  let scratch = Scratch::with_settings("");
  scratch.file("README.md", "Kontur\n");
  let failed = init_repository(&scratch.git, &scratch.folder(), FirstCommit::Files);
  let Err(InitError::NoIdentity(said)) = failed else {
    panic!("expected the identity failure, got {failed:?}");
  };
  assert!(!said.is_empty(), "git's own words are kept");
  assert_eq!(
    read_local(&scratch.git, &scratch.folder()),
    LocalRepository::NoCommits {
      branch: Some(FALLBACK_BRANCH.to_owned())
    },
    "the repository is kept without a commit"
  );
  // A mark inside the repository shows that Try again does not make it again.
  let mark = scratch.folder().join(".git").join("orchester-test-mark");
  fs::write(&mark, "kept").expect("a mark");
  assert_eq!(
    create_repository(&scratch.git, &scratch.folder()),
    Err(InitError::AlreadyRepository)
  );
  write_settings(
    &scratch.git.global,
    "[user]\n\tname = Test Author\n\temail = author@example.com\n",
  );
  let made = make_first_commit(&scratch.git, &scratch.folder(), FirstCommit::Files).expect("the commit is made");
  assert_eq!(made.files, 1);
  assert!(mark.exists(), "the repository was kept");
  assert_eq!(scratch.ask(&["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn git_init_detects_a_repository_without_commits() {
  let scratch = Scratch::new();
  assert_eq!(
    create_repository(&scratch.git, &scratch.folder()),
    Ok(FALLBACK_BRANCH.to_owned())
  );
  assert_eq!(
    read_local(&scratch.git, &scratch.folder()),
    LocalRepository::NoCommits {
      branch: Some(FALLBACK_BRANCH.to_owned())
    }
  );
}

#[test]
fn git_init_says_git_is_missing() {
  let mut scratch = Scratch::new();
  scratch.git.program = Some(scratch.dir.path().join("no-git").join("git"));
  assert_eq!(read_local(&scratch.git, &scratch.folder()), LocalRepository::GitMissing);
  assert_eq!(survey(&scratch.git, &scratch.folder()), Err(SurveyError::GitMissing));
  assert_eq!(
    init_repository(&scratch.git, &scratch.folder(), FirstCommit::Empty),
    Err(InitError::GitMissing)
  );
  assert!(!scratch.folder().join(".git").exists());
}
