//! Add project › *Create a git repository here* (`OdlXb`): a folder that is not a git repository is made one in
//! place, with its files or an empty first commit, and the project can then be created; a commit git cannot make
//! for want of an author says so and Try again makes only the commit; git that is not installed holds Create back
//! and says how to install it. Real git runs in folders under this worktree's `.tmp/`, with git's settings in a file
//! of each test's own (`GIT_CONFIG_GLOBAL`, no system settings, no author from the environment), so the person's
//! git settings are never read or written by what the page runs.

mod support;

use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};
use orchester_exec::{ProcessRunner, programs::Programs};
use support::{Data, Window, stand_ins::StandIns};
use tempfile::TempDir;

const AUTHOR: &str = "[user]\n\tname = Test Author\n\temail = author@example.com\n";

/// git with the settings file `global` only, looking for no repository above `ceiling` (the worktree's own), or,
/// with `program`, a git that is not installed.
struct Isolated {
  global: PathBuf,
  ceiling: PathBuf,
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

/// A folder `kontur` of the test's own, git's settings beside it.
struct Scratch {
  dir: TempDir,
}

impl Scratch {
  fn new(settings: &str) -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/git-init/app-tests");
    fs::create_dir_all(&root).expect("a scratch folder");
    let scratch = Self {
      dir: tempfile::tempdir_in(root).expect("a folder of the test's own"),
    };
    scratch.settings(settings);
    fs::create_dir_all(scratch.folder()).expect("the folder");
    scratch
  }

  /// git's settings are `settings`; `useConfigOnly` keeps git from guessing an author from the computer.
  fn settings(&self, settings: &str) {
    fs::write(self.global(), format!("[user]\n\tuseConfigOnly = true\n{settings}")).expect("git's settings");
  }

  fn global(&self) -> PathBuf {
    self.dir.path().join("gitconfig")
  }

  fn folder(&self) -> PathBuf {
    self.dir.path().join("kontur")
  }

  fn file(&self, name: &str) {
    let path = self.folder().join(name);
    fs::create_dir_all(path.parent().expect("a file in the folder")).expect("its folder");
    fs::write(path, name).expect("a file");
  }

  fn programs(&self, program: Option<PathBuf>) -> Programs {
    Programs::with_runner(Arc::new(Isolated {
      global: self.global(),
      ceiling: self.dir.path().to_path_buf(),
      program,
    }))
  }
}

/// The window on Add project with git run by `programs`, tall enough to show the whole block.
fn add_project(programs: Programs) -> (Window, StandIns) {
  let data = Data::new();
  let launch = data.launch().with_programs(programs);
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  window.tree.resize(1440, 1800);
  window.navigation().go(Route::AddProject(None));
  window.until("Add project", |window| {
    window.tree.get_element_by_id("screen-add_project").is_some()
  });
  (window, stand_ins)
}

/// Chooses `folder` and waits until the block says `status`.
fn choose(window: &mut Window, stand_ins: &StandIns, folder: &Path, status: &str) {
  stand_ins.choose(Some(folder.to_path_buf()));
  window.press("choose-folder");
  until_status(window, status);
}

fn until_status(window: &mut Window, status: &str) {
  window.until_within(Duration::from_secs(30), status, |window| {
    window
      .texts("github-detection-status")
      .is_some_and(|said| said == [status])
  });
}

/// Waits until the files a first commit would add are counted, and returns what the block says of them.
fn counted(window: &mut Window) -> String {
  window.until_within(Duration::from_secs(30), "the files counted", |window| {
    !window.text("github-detection-properties").contains("Counting files…")
  });
  window.text("github-detection-properties")
}

/// Creates the project named Kontur in the chosen folder and waits for its Overview.
fn create_the_project(window: &mut Window, folder: &Path) {
  window.set("project-name", "Kontur");
  window.press("create-project");
  window.until("the project to open", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Overview(_))
    )
  });
  let workspace = window.launch.workspace().expect("the session").clone();
  let projects = workspace.projects().expect("the list");
  assert_eq!(projects.len(), 1);
  assert_eq!(projects[0].repo_root, folder);
}

/// The words inside `id` one space apart: a command row shows each word on its own.
fn words(window: &mut Window, id: &str) -> String {
  window.text(id).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn project_count(window: &Window) -> usize {
  let workspace = window.launch.workspace().expect("the session").clone();
  workspace.projects().expect("the list").len()
}

#[test]
fn git_init_makes_the_folder_a_repository_with_its_files_and_the_project_can_be_created() {
  let scratch = Scratch::new(AUTHOR);
  for file in [".gitignore", "README.md", "src/main.rs"] {
    scratch.file(file);
  }
  fs::write(scratch.folder().join(".gitignore"), "*.log\n").expect("a .gitignore");
  scratch.file("run.log");
  let (mut window, stand_ins) = add_project(scratch.programs(None));
  choose(&mut window, &stand_ins, &scratch.folder(), "Not a repository");
  let properties = counted(&mut window);
  assert!(
    properties.contains("Files 3 files · a .gitignore is in the folder"),
    "{properties}"
  );
  let block = window.text("github-detection");
  for said in [
    "git init on the branch main, since init.defaultBranch is not set.",
    "Adds 3 files. The .gitignore in the folder is honoured: what it lists stays out of git.",
    "Review the files git will add; a .gitignore keeps files out.",
  ] {
    assert!(block.contains(said), "{said}: {block}");
  }
  assert!(
    words(&mut window, "git-init-fallback").contains("git init --initial-branch=main"),
    "the terminal makes the branch the button makes"
  );
  window.set("project-name", "Kontur");
  window.press("create-project");
  assert_eq!(project_count(&window), 0, "Create waits for a repository");
  window.press("git-init-create");
  until_status(&mut window, "Local only");
  assert_eq!(
    window.text("git-init-done"),
    "Created here with a first commit of 3 files."
  );
  let properties = window.text("github-detection-properties");
  assert!(
    properties.contains("Git Git repository · main · 1 commit"),
    "{properties}"
  );
  assert_eq!(
    window.text("add-project-status"),
    "Creates the project in the repository’s .orchester folder and opens it."
  );
  create_the_project(&mut window, &scratch.folder());
}

#[test]
fn git_init_an_empty_folder_takes_an_empty_first_commit() {
  let scratch = Scratch::new(AUTHOR);
  let (mut window, stand_ins) = add_project(scratch.programs(None));
  choose(&mut window, &stand_ins, &scratch.folder(), "Not a repository");
  let properties = counted(&mut window);
  assert!(properties.contains("Files None · the folder is empty"), "{properties}");
  assert_eq!(
    window.text("git-init-first-files-reason"),
    "The folder is empty: there are no files to commit."
  );
  assert!(
    words(&mut window, "git-init-fallback").contains("git commit --allow-empty -m \"Initial commit\""),
    "the terminal makes the same empty commit"
  );
  window.press("git-init-create");
  until_status(&mut window, "Local only");
  assert_eq!(window.text("git-init-done"), "Created here with an empty first commit.");
}

#[test]
fn git_init_without_an_author_says_so_and_try_again_makes_only_the_commit() {
  let scratch = Scratch::new("");
  scratch.file("README.md");
  let (mut window, stand_ins) = add_project(scratch.programs(None));
  choose(&mut window, &stand_ins, &scratch.folder(), "Not a repository");
  counted(&mut window);
  window.press("git-init-create");
  until_status(&mut window, "No commits");
  window.until("the failure", |window| window.texts("git-init-error").is_some());
  assert_eq!(window.text("github-detection-heading"), "The first commit failed");
  assert_eq!(window.text("git-init-error"), "git commit failed. git said:");
  assert!(window.texts("git-init-output").is_some(), "git's own words");
  assert!(
    words(&mut window, "git-identity-email").contains("git config --global user.email \"you@example.com\""),
    "how to tell git who the author is"
  );
  assert_eq!(
    window.text("add-project-status"),
    "The project folder needs a first commit to continue."
  );
  window.press("create-project");
  assert_eq!(project_count(&window), 0, "Create waits for a commit");
  scratch.settings(AUTHOR);
  window.until("Try again", |window| {
    window
      .texts("git-init-create")
      .is_some_and(|said| said == ["Try again"])
  });
  window.until_within(Duration::from_secs(30), "the files counted", |window| {
    window.text("github-detection-properties").contains("Files 1 file")
  });
  window.press("git-init-create");
  until_status(&mut window, "Local only");
  assert_eq!(
    window.text("git-init-done"),
    "Created here with a first commit of 1 file."
  );
  create_the_project(&mut window, &scratch.folder());
}

#[test]
fn git_init_needs_git_and_says_how_to_install_it() {
  let scratch = Scratch::new(AUTHOR);
  let missing = scratch.dir.path().join("no-git").join("git");
  let (mut window, stand_ins) = add_project(scratch.programs(Some(missing)));
  choose(&mut window, &stand_ins, &scratch.folder(), "git not found");
  assert_eq!(
    window.text("github-detection-heading"),
    "git is not installed on this computer"
  );
  assert_eq!(
    window.text("git-init-unavailable"),
    "Create a git repository here needs git on this computer."
  );
  let command = words(&mut window, "git-install-command");
  #[cfg(target_os = "windows")]
  assert!(
    command.contains("winget install --id Git.Git -e --source winget"),
    "{command}"
  );
  #[cfg(target_os = "macos")]
  assert!(command.contains("xcode-select --install"), "{command}");
  assert_eq!(
    window.text("add-project-status"),
    "Install git to continue: runs work in git worktrees."
  );
  window.set("project-name", "Kontur");
  window.press("create-project");
  assert_eq!(project_count(&window), 0, "Create waits for git");
  assert!(!scratch.folder().join(".git").exists(), "nothing was made");
}
