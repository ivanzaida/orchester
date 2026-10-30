//! Which agent CLI the start page warns of, and the file a person chooses for it: first starts and listed projects,
//! what does not count as needing a CLI, and that a file which does not answer as the CLI changes nothing. The search
//! path is passed in, so no test depends on the machine's `PATH`.
//!
//! `cargo test -p orchester-workspace --test missing_cli missing_cli`

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, Connection, ConnectionStatus, GitHubConnection, GitHubHost},
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_exec::search_path::{ChosenProgramError, SearchBasis, SearchPath};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  connections::{
    choose_program::{ChooseProgramError, choose_program},
    missing_cli::{MissingCli, missing_cli_on},
  },
  project_setup::NewProject,
  workspace::{ProjectEntry, Workspace},
};
use tempfile::TempDir;

fn workspace(dir: &TempDir) -> Workspace {
  Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("an application database"))
}

/// The project `name` set up in a repository of `dir`.
fn add_project(dir: &TempDir, workspace: &Workspace, (key, name): (&str, &str)) -> PathBuf {
  let repo = dir.path().join(format!("repo-{key}"));
  fs::create_dir_all(repo.join(".git")).expect("a repository");
  workspace
    .create(NewProject {
      project: Project::new(
        ProjectId::generate(),
        ProjectKey::new(key).expect("a key"),
        Name::new(name).expect("a name"),
      ),
      repo_root: repo.clone(),
    })
    .expect("the project is set up");
  repo
}

fn add_connection(repo: &Path, provider: AgentProvider, program: PathBuf) -> ConnectionId {
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(&format!("{provider:?}")).expect("a name"),
    provider,
    program,
    Timestamp::now(),
  )
  .expect("an absolute program");
  ProjectDb::open(repo)
    .expect("the project's data")
    .create_connection(&connection)
    .expect("stored");
  connection.id()
}

fn stored(repo: &Path, id: ConnectionId) -> Connection {
  ProjectDb::open(repo)
    .expect("the project's data")
    .connection(id)
    .expect("read")
    .expect("stored")
}

/// The search path of one empty folder of `dir`, which it makes.
fn empty_search(dir: &TempDir) -> (PathBuf, SearchPath) {
  let folder = dir.path().join("search");
  fs::create_dir_all(&folder).expect("a folder");
  let search = SearchPath::merge(Some(folder.as_os_str()), &[], &[]);
  (folder, search)
}

fn entries(workspace: &Workspace) -> Vec<ProjectEntry> {
  workspace.entries().expect("the list reads")
}

fn missing(workspace: &Workspace, search: &SearchPath) -> Option<MissingCli> {
  missing_cli_on(workspace, &entries(workspace), search, &SearchBasis::Path).expect("the rule reads")
}

fn expect_missing(workspace: &Workspace, search: &SearchPath) -> MissingCli {
  missing(workspace, search).expect("the callout shows")
}

/// A stand-in `name` in `folder` that answers `--version` with `version_line`.
#[cfg(windows)]
fn write_program(folder: &Path, name: &str, version_line: &str) -> PathBuf {
  let program = folder.join(format!("{name}.cmd"));
  let escaped = version_line.replace('(', "^(").replace(')', "^)");
  let script = format!(
    "@echo off\r\nif \"%1\"==\"--version\" goto version\r\nexit /b 1\r\n:version\r\necho {escaped}\r\nexit /b 0\r\n"
  );
  fs::write(&program, script).expect("the stand-in is written");
  program
}

/// A stand-in `name` in `folder` that answers `--version` with `version_line`.
#[cfg(not(windows))]
fn write_program(folder: &Path, name: &str, version_line: &str) -> PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let program = folder.join(name);
  let script = format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo '{version_line}'; exit 0; fi\nexit 1\n");
  fs::write(&program, script).expect("the stand-in is written");
  fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
  program
}

fn write_claude(folder: &Path) -> PathBuf {
  write_program(folder, "claude", "2.1.296 (Claude Code)")
}

fn write_codex(folder: &Path) -> PathBuf {
  write_program(folder, "codex", "codex-cli 0.159.2")
}

/// A folder of `dir` that is on no search path, for what a person chooses.
fn elsewhere(dir: &TempDir) -> PathBuf {
  let folder = dir.path().join("elsewhere");
  fs::create_dir_all(&folder).expect("a folder");
  folder
}

/// A program path that is gone.
fn gone(dir: &TempDir) -> PathBuf {
  dir.path().join("gone").join("claude.exe")
}

#[test]
fn missing_cli_at_a_first_start_names_claude_and_the_folders_searched() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (folder, search) = empty_search(&dir);
  let missing = expect_missing(&workspace, &search);
  assert_eq!(missing.provider, AgentProvider::ClaudeCodeCli);
  assert_eq!(missing.program, "claude");
  assert_eq!(missing.searched, [folder.display().to_string()]);
  assert_eq!(missing.basis, SearchBasis::Path);
  assert!(missing.needing.is_empty(), "a first start has no connection to fix");
}

#[test]
fn missing_cli_at_a_first_start_is_nothing_with_either_cli_on_the_search_path() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (folder, search) = empty_search(&dir);
  write_codex(&folder);
  assert_eq!(missing(&workspace, &search), None, "a found codex is enough to start");
}

#[test]
fn missing_cli_comes_back_when_the_remembered_file_is_gone() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let chosen = write_claude(&elsewhere(&dir));
  let warned = expect_missing(&workspace, &search);
  assert_eq!(
    choose_program(&workspace, &warned, &chosen).expect("it answers as the CLI"),
    Vec::<ProjectId>::new()
  );
  assert_eq!(missing(&workspace, &search), None, "the file is remembered");
  fs::remove_file(&chosen).expect("the file goes");
  assert!(missing(&workspace, &search).is_some(), "the callout returns");
}

#[test]
fn missing_cli_lists_the_connections_whose_program_is_gone() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  let notes = add_project(&dir, &workspace, ("NOT", "Notes"));
  let first = add_connection(&atlas, AgentProvider::ClaudeCodeCli, gone(&dir));
  let second = add_connection(&notes, AgentProvider::ClaudeCodeCli, gone(&dir));
  let missing = expect_missing(&workspace, &search);
  assert_eq!(missing.program, "claude");
  let needing: Vec<(ConnectionId, &str)> = missing
    .needing
    .iter()
    .map(|needing| (needing.connection, needing.name.as_str()))
    .collect();
  assert_eq!(needing.len(), 2);
  assert!(needing.contains(&(first, "Atlas")), "{needing:?}");
  assert!(needing.contains(&(second, "Notes")), "{needing:?}");
}

#[test]
fn missing_cli_is_nothing_with_a_copy_on_the_search_path() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (folder, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  add_connection(&atlas, AgentProvider::ClaudeCodeCli, gone(&dir));
  write_claude(&folder);
  assert_eq!(missing(&workspace, &search), None, "claiming not found would be false");
}

#[test]
fn missing_cli_names_claude_before_codex_and_codex_once_claude_is_there() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (folder, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  add_connection(
    &atlas,
    AgentProvider::CodexCli,
    dir.path().join("gone").join("codex.exe"),
  );
  add_connection(&atlas, AgentProvider::ClaudeCodeCli, gone(&dir));
  assert_eq!(expect_missing(&workspace, &search).program, "claude");
  write_claude(&folder);
  assert_eq!(expect_missing(&workspace, &search).program, "codex");
}

#[test]
fn missing_cli_ignores_a_present_program_and_github() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  let present = write_claude(&elsewhere(&dir));
  add_connection(&atlas, AgentProvider::ClaudeCodeCli, present);
  let github = GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("a name"),
    GitHubHost::github_com(),
    dir.path().join("gone").join("gh.exe"),
    Timestamp::now(),
  )
  .expect("an absolute program");
  ProjectDb::open(&atlas)
    .expect("the project's data")
    .create_connection(&github)
    .expect("stored");
  assert_eq!(missing(&workspace, &search), None);
}

#[test]
fn missing_cli_with_a_listed_project_and_no_connection_is_nothing() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  add_project(&dir, &workspace, ("ATL", "Atlas"));
  assert_eq!(missing(&workspace, &search), None, "no connection needs a CLI");
}

#[test]
fn missing_cli_skips_a_missing_folder_and_data_it_cannot_read() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let moved = add_project(&dir, &workspace, ("MOV", "Moved"));
  add_connection(&moved, AgentProvider::ClaudeCodeCli, gone(&dir));
  fs::remove_dir_all(&moved).expect("the folder goes");
  let newer = add_project(&dir, &workspace, ("NEW", "Newer"));
  add_connection(&newer, AgentProvider::ClaudeCodeCli, gone(&dir));
  rusqlite::Connection::open(newer.join(".orchester").join("orchester.db"))
    .expect("the project's data")
    .pragma_update(None, "user_version", 999)
    .expect("a newer schema version");
  assert_eq!(missing(&workspace, &search), None, "their rows already say why");
}

/// Reading a network path would hand that server the person's credentials.
#[cfg(windows)]
#[test]
fn missing_cli_does_not_count_a_network_program_as_gone() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  add_connection(
    &atlas,
    AgentProvider::ClaudeCodeCli,
    PathBuf::from(r"\\server.invalid\share\claude.exe"),
  );
  assert_eq!(missing(&workspace, &search), None);
}

#[test]
fn missing_cli_chosen_file_repoints_every_needing_connection_and_records_its_check() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  let notes = add_project(&dir, &workspace, ("NOT", "Notes"));
  let first = add_connection(&atlas, AgentProvider::ClaudeCodeCli, gone(&dir));
  let second = add_connection(&notes, AgentProvider::ClaudeCodeCli, gone(&dir));
  let chosen = write_claude(&elsewhere(&dir));
  let warned = expect_missing(&workspace, &search);
  let fixed = choose_program(&workspace, &warned, &chosen).expect("it answers as the CLI");
  assert_eq!(fixed.len(), 2);
  for (repo, id) in [(&atlas, first), (&notes, second)] {
    let connection = stored(repo, id);
    assert_eq!(connection.executable(), Some(chosen.as_path()));
    assert_ne!(
      connection.status(),
      ConnectionStatus::NotChecked,
      "its check is recorded"
    );
    assert_eq!(connection.version(), Some("2.1.296"));
  }
  assert_eq!(missing(&workspace, &search), None, "the callout goes");
}

#[test]
fn missing_cli_wrong_file_writes_nothing_and_remembers_nothing() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let atlas = add_project(&dir, &workspace, ("ATL", "Atlas"));
  let program = gone(&dir);
  let id = add_connection(&atlas, AgentProvider::ClaudeCodeCli, program.clone());
  let wrong = write_program(&elsewhere(&dir), "gh", "gh version 2.62.0");
  let warned = expect_missing(&workspace, &search);
  let refused = choose_program(&workspace, &warned, &wrong).expect_err("it is not the CLI");
  assert!(
    matches!(&refused, ChooseProgramError::NotTheCli { file, program: "claude" } if file == &wrong),
    "{refused:?}"
  );
  assert_eq!(refused.message().key, "connections.errors.notTheCli");
  assert_eq!(stored(&atlas, id).executable(), Some(program.as_path()));
  assert_eq!(
    missing(&workspace, &search).map(|missing| missing.needing.len()),
    Some(1),
    "the connection still needs the CLI"
  );
  let keys = workspace.ui_state_keys("orchester-app.agentProgram.").expect("keys");
  assert!(keys.is_empty(), "nothing is remembered: {keys:?}");
}

#[test]
fn missing_cli_refuses_a_file_that_is_not_there() {
  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let warned = expect_missing(&workspace, &search);
  let file = dir.path().join("no-such-file");
  let refused = choose_program(&workspace, &warned, &file).expect_err("nothing is there");
  assert!(
    matches!(refused, ChooseProgramError::Refused(ChosenProgramError::Missing(_))),
    "{refused:?}"
  );
  assert_eq!(refused.message().key, "appMcp.errors.programMissing");
}

/// Windows has no execute permission: whether a file runs is the check's to find.
#[cfg(not(windows))]
#[test]
fn missing_cli_refuses_a_file_that_cannot_run() {
  use std::os::unix::fs::PermissionsExt;

  let dir = TempDir::new().expect("a folder");
  let workspace = workspace(&dir);
  let (_, search) = empty_search(&dir);
  let warned = expect_missing(&workspace, &search);
  let file = write_claude(&elsewhere(&dir));
  fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("no longer runs");
  let refused = choose_program(&workspace, &warned, &file).expect_err("it cannot run");
  assert!(
    matches!(
      refused,
      ChooseProgramError::Refused(ChosenProgramError::NotExecutable(_))
    ),
    "{refused:?}"
  );
}
