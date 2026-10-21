//! Making a folder a git repository in place (`repository::init`), with real git in folders of the tests' own under
//! the worktree's `.tmp/`. git's settings are a file of each test's own (`GIT_CONFIG_GLOBAL`, no system settings,
//! no identity from the environment), so the person's git settings are never read or written, and git looks for
//! no repository above each test's folder (`GIT_CEILING_DIRECTORIES`), so the worktree's own is not found.

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};
use orchester_exec::ProcessRunner;
use orchester_workspace::repository::init::{
  Commits, FALLBACK_BRANCH, FirstCommit, InitError, LocalRepository, SurveyError, create_repository, init_repository,
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

/// git with the settings file `global` only, looking for no repository above `ceiling`.
struct Isolated {
  global: PathBuf,
  ceiling: PathBuf,
  /// Runs this program in place of git: one that does not exist stands for git not installed.
  program: Option<PathBuf>,
  /// Variables of the caller's environment, as a hook that started Orchester could pass on.
  inherited: Vec<(&'static str, String)>,
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
    .with_env("GIT_CONFIG_NOSYSTEM", "1")
    .with_env("GIT_CEILING_DIRECTORIES", &self.ceiling.to_string_lossy());
    for variable in IDENTITY_VARIABLES {
      isolated = isolated.without_env(variable);
    }
    // As this process's environment would hold them: each reaches git unless the command leaves it out.
    for (name, value) in &self.inherited {
      if !command.env_removed().iter().any(|removed| removed == name) {
        isolated = isolated.with_env(name, value);
      }
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
      git: Isolated {
        global,
        ceiling: dir.path().to_path_buf(),
        program: None,
        inherited: Vec::new(),
      },
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
  assert_eq!(made.files, Some(0));
  assert_eq!(scratch.ask(&["symbolic-ref", "--short", "HEAD"]), FALLBACK_BRANCH);
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
      commits: Commits::One
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
  assert_eq!(made.files, Some(3));
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
  init_repository(&scratch.git, &scratch.folder(), FirstCommit::Empty).expect("the repository is made");
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
  assert_eq!(made.files, Some(1));
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

#[test]
fn git_init_refuses_an_empty_first_commit_over_staged_files() {
  let scratch = Scratch::new();
  scratch.file("README.md", "Kontur\n");
  scratch.file("notes.txt", "draft\n");
  scratch.ask(&["init", "--quiet"]);
  scratch.ask(&["add", "README.md"]);
  let surveyed = survey(&scratch.git, &scratch.folder()).expect("the folder is surveyed");
  assert_eq!((surveyed.files, surveyed.staged), (2, 1));
  assert_eq!(
    make_first_commit(&scratch.git, &scratch.folder(), FirstCommit::Empty),
    Err(InitError::Staged)
  );
  assert_eq!(
    scratch.ask(&["ls-files", "--cached"]),
    "README.md",
    "what the person staged is kept"
  );
  let made = make_first_commit(&scratch.git, &scratch.folder(), FirstCommit::Files).expect("the commit is made");
  assert_eq!(made.files, Some(2));
}

#[test]
fn git_init_says_which_repository_a_folder_is_inside_of() {
  let scratch = Scratch::new();
  scratch.ask(&["init", "--quiet"]);
  let inner = scratch.folder().join("inner");
  fs::create_dir_all(&inner).expect("a folder inside the repository");
  assert_eq!(read_local(&scratch.git, &inner), LocalRepository::NotRepository);
  let surveyed = survey(&scratch.git, &inner).expect("the folder is surveyed");
  let enclosing = surveyed.enclosing.expect("the repository the folder is inside of");
  assert_eq!(
    fs::canonicalize(enclosing).expect("the repository's top"),
    fs::canonicalize(scratch.folder()).expect("the folder")
  );
  assert_eq!(
    survey(&scratch.git, &scratch.folder())
      .expect("the folder is surveyed")
      .enclosing,
    None,
    "a repository's top is inside of none"
  );
}

#[test]
fn git_init_works_on_the_folder_whatever_repository_the_environment_names() {
  let scratch = Scratch::new();
  let other = scratch.dir.path().join("other");
  fs::create_dir_all(&other).expect("another repository");
  let in_other = |args: &[&str]| {
    let output = scratch
      .git
      .run(&Command::new(Path::new("git"), args.iter().copied()).with_current_dir(&other))
      .expect("git runs");
    assert!(output.success(), "git {args:?}: {}", output.stderr);
    output.stdout.trim().to_owned()
  };
  in_other(&["init", "--quiet"]);
  fs::write(other.join("kept.txt"), "kept\n").expect("a file");
  in_other(&["add", "kept.txt"]);
  in_other(&["commit", "--quiet", "-m", "Theirs"]);
  fs::write(other.join("staged.txt"), "staged\n").expect("a file");
  in_other(&["add", "staged.txt"]);
  let (head, index) = (in_other(&["rev-parse", "HEAD"]), in_other(&["ls-files", "--stage"]));
  let git_dir = other.join(".git");
  let poisoned = Isolated {
    global: scratch.git.global.clone(),
    ceiling: scratch.git.ceiling.clone(),
    program: None,
    inherited: vec![
      ("GIT_DIR", git_dir.to_string_lossy().into_owned()),
      ("GIT_WORK_TREE", other.to_string_lossy().into_owned()),
      ("GIT_INDEX_FILE", git_dir.join("index").to_string_lossy().into_owned()),
    ],
  };
  scratch.file("README.md", "Kontur\n");
  assert_eq!(read_local(&poisoned, &scratch.folder()), LocalRepository::NotRepository);
  let surveyed = survey(&poisoned, &scratch.folder()).expect("the folder is surveyed");
  assert_eq!((surveyed.files, surveyed.enclosing), (1, None));
  init_repository(&poisoned, &scratch.folder(), FirstCommit::Files).expect("the repository is made");
  assert_eq!(scratch.committed(), ["README.md"]);
  assert_eq!(
    in_other(&["rev-parse", "HEAD"]),
    head,
    "the other repository has no new commit"
  );
  assert_eq!(in_other(&["ls-files", "--stage"]), index, "its index is as it was");
  let empty = scratch.dir.path().join("empty");
  fs::create_dir_all(&empty).expect("an empty folder");
  create_repository(&poisoned, &empty).expect("the repository is made");
  make_first_commit(&poisoned, &empty, FirstCommit::Empty).expect("the commit is made");
  assert_eq!(
    in_other(&["ls-files", "--stage"]),
    index,
    "an empty first commit leaves its index alone"
  );
}
