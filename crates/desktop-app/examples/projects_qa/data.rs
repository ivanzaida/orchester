//! What the projects QA window shows, all under the QA folder: an application database with the seed's
//! `task-states` Kontur and projects in each state All projects names (opened minutes to weeks ago, one whose
//! folder is gone, one whose data a newer Orchester wrote), the folders Add project is shown with (a repository
//! with a GitHub remote, one without a remote, one on another host, a folder of files that is not a repository and
//! an empty one) and a repository that keeps no project for *Open project folder…*. The stand-in `gh` is built here
//! too, and the window's git settings, so *Create a git repository here* never reads or writes the person's.

use std::{
  error::Error,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};

use super::kontur;

/// The seed's scenario for Kontur: tasks in every state, so it has runs that need the person.
const SCENARIO: &str = "task-states";
/// The other projects of the list: name, key, folder and how long ago each was opened, in minutes.
const LISTED: [(&str, &str, &str, i64); 5] = [
  ("Atlas", "ATL", "atlas", 12),
  ("Orchester", "ORC", "orchester", 120),
  ("Personal website", "PER", "personal-website", 300),
  ("Notes", "NOT", "notes", 9 * 24 * 60),
  ("Legacy CRM", "LEG", "legacy-crm", 27 * 24 * 60),
];
/// The project whose folder is gone, and the one whose data a newer Orchester wrote.
const GONE: &str = "notes";
const NEWER: &str = "legacy-crm";
/// The author of the repositories made here, and of first commits the window makes.
const AUTHOR: &str = "[user]\n\tname = QA Author\n\temail = qa@example.com\n";
/// The GitHub repository the setup folder's remote names (the design's).
const GITHUB_REMOTE: &str = "https://github.com/ivanzaida/kontur.git";
const OTHER_REMOTE: &str = "https://gitlab.com/ivanzaida/kontur.git";

/// The folders of one QA start, under `root`.
pub struct Folders {
  pub root: PathBuf,
}

impl Folders {
  /// The stand-in `gh` on the window's `PATH`.
  pub fn path(&self) -> PathBuf {
    self.root.join("path")
  }

  /// The stand-in `gh` *Choose program…* picks, off the `PATH`.
  pub fn program(&self) -> PathBuf {
    self.root.join("program").join(gh_name())
  }

  pub fn app(&self) -> PathBuf {
    self.root.join("app")
  }

  /// A folder Add project or *Open project folder…* is shown with.
  pub fn folder(&self, name: &str) -> PathBuf {
    self.root.join("folders").join(name)
  }

  /// The seeded Kontur's repository.
  pub fn kontur(&self) -> PathBuf {
    self.root.join("projects").join("kontur")
  }

  /// The window's git settings (`GIT_CONFIG_GLOBAL`).
  pub fn git_settings(&self) -> PathBuf {
    self.root.join("gitconfig")
  }
}

/// Writes the window's git settings: only `useConfigOnly`, so git never guesses an author from the computer, and
/// the QA author unless `author` is false.
pub fn git_settings(folders: &Folders, author: bool) -> Result<(), Box<dyn Error>> {
  let who = if author { AUTHOR } else { "" };
  fs::write(folders.git_settings(), format!("[user]\n\tuseConfigOnly = true\n{who}"))?;
  Ok(())
}

fn gh_name() -> String {
  format!("gh{}", std::env::consts::EXE_SUFFIX)
}

/// Builds the stand-in `gh` into the `PATH` folder and the program folder, answering as `state`.
pub fn stand_in(folders: &Folders, state: &str) -> Result<(), Box<dyn Error>> {
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/projects_qa/gh.rs");
  for folder in [folders.path(), folders.root.join("program")] {
    fs::create_dir_all(&folder)?;
    let output = Command::new("rustc")
      .arg(&source)
      .args(["--edition", "2024", "-o"])
      .arg(folder.join(gh_name()))
      .current_dir(&folder)
      .output()?;
    if !output.status.success() {
      return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    fs::write(folder.join("state"), state)?;
  }
  Ok(())
}

/// The application database the window opens, and what the start prints of it.
pub struct Seeded {
  pub database: PathBuf,
  pub shown: String,
}

/// Seeds Kontur and the listed projects.
pub fn projects(folders: &Folders) -> Result<Seeded, Box<dyn Error>> {
  let scenario = kontur::scenarios::named(SCENARIO).ok_or("the seed has the task-states scenario")?;
  let now = Timestamp::now();
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: folders.app(),
    repo: folders.kontur(),
    scenario,
    fake_codex: None,
    now,
  })?;
  let mut app = AppDb::open(&seeded.app_database)?;
  // The signed-in account's projects, or the next one's to sign in.
  let owner = app.cloud_account()?.map(|account| account.account);
  for (name, key, folder, minutes) in LISTED {
    let repo = folders.root.join("projects").join(folder);
    git_init(&repo)?;
    let project = Project::new(ProjectId::generate(), ProjectKey::new(key)?, Name::new(name)?);
    ProjectDb::create(&repo, project.clone())?;
    let opened = Timestamp::from_unix_millis(now.unix_millis() - minutes * 60 * 1000);
    app.register_project(&project, &repo, opened, owner.as_ref())?;
    if folder == NEWER {
      rusqlite::Connection::open(repo.join(".orchester").join("orchester.db"))?.pragma_update(
        None,
        "user_version",
        999,
      )?;
    }
    if folder == GONE {
      fs::remove_dir_all(&repo)?;
    }
  }
  let tasks: Vec<String> = seeded
    .tasks
    .iter()
    .map(|task| format!("{} {} ({})", task.key, task.title, task.state))
    .collect();
  Ok(Seeded {
    database: seeded.app_database,
    shown: format!(
      "{} {} in {}: {}",
      scenario.name(),
      seeded.project,
      seeded.repo.display(),
      tasks.join("; ")
    ),
  })
}

/// An application database with no project: a first launch.
pub fn empty(folders: &Folders) -> Result<Seeded, Box<dyn Error>> {
  fs::create_dir_all(folders.app())?;
  let database = orchester_store::location::app_database_beside(&folders.app().join("orchester-app"));
  AppDb::open(&database)?;
  Ok(Seeded {
    database,
    shown: "no project".to_owned(),
  })
}

/// The folders Add project and *Open project folder…* are shown with.
pub fn folders(folders: &Folders) -> Result<(), Box<dyn Error>> {
  let github = folders.folder("kontur");
  git_init(&github)?;
  git(&github, &["remote", "add", "origin", GITHUB_REMOTE])?;
  git_init(&folders.folder("local"))?;
  let other = folders.folder("other-host");
  git_init(&other)?;
  git(&other, &["remote", "add", "origin", OTHER_REMOTE])?;
  let notes = folders.folder("kontur-notes");
  for (file, text) in [
    (".gitignore", "*.log\n"),
    ("README.md", "# Kontur\n"),
    ("src/main.rs", "fn main() {}\n"),
    ("run.log", "ignored\n"),
  ] {
    let path = notes.join(file);
    fs::create_dir_all(path.parent().ok_or("a file in the folder")?)?;
    fs::write(path, text)?;
  }
  fs::create_dir_all(folders.folder("kontur-empty"))?;
  git_init(&folders.folder("scratch"))?;
  Ok(())
}

/// A repository with an empty first commit, as runs need one.
fn git_init(repo: &Path) -> Result<(), Box<dyn Error>> {
  fs::create_dir_all(repo)?;
  git(repo, &["init", "--quiet"])?;
  git(
    repo,
    &[
      "-c",
      "user.name=QA Author",
      "-c",
      "user.email=qa@example.com",
      "-c",
      "commit.gpgsign=false",
      "commit",
      "--quiet",
      "--allow-empty",
      "-m",
      "Initial commit",
    ],
  )
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
  let output = Command::new("git").arg("-C").arg(repo).args(args).output()?;
  if output.status.success() {
    Ok(())
  } else {
    Err(String::from_utf8_lossy(&output.stderr).into_owned().into())
  }
}
