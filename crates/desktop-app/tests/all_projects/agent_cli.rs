//! That an agent CLI is not found (`SIbdo`, `U4QvYO` in frame 14 `mruEH`): the callout and the fix row on the welcome
//! and above the list, choosing a file, *Check again*, and that a project still opens. The search is a stand-in: a test
//! cannot change its process's `PATH` for itself alone, and the macOS wording must be reachable on Windows.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_app::routes::{OverviewTab, ProjectPage, Route};
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_exec::search_path::{SearchBasis, SearchPath};
use orchester_store::ProjectDb;
use tempfile::TempDir;

use super::{
  all_projects, row,
  support::{Data, Window, project_id, stand_ins::StandIns},
};

const CLI: &str = "start-cli";
const CALLOUT: &str = "start-cli-callout";
const ERROR: &str = "start-cli-error";
/// The welcome's footer (`N3VzU`), under what it centres.
const FOOTER_HEIGHT: f32 = 56.0;

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

fn search_in(folder: &Path) -> SearchPath {
  SearchPath::merge(Some(folder.as_os_str()), &[], &[])
}

fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn shows(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// The window on Kontur, whose Claude connection runs a file that is gone, at All projects with the warning showing;
/// and that connection.
fn kontur_without_claude() -> (Window, StandIns, ConnectionId, PathBuf) {
  let data = Data::new();
  let launch = data.launch();
  let kontur = data.add_kontur(launch.workspace().expect("the session started"));
  let program = data.dir.path().join("gone").join("claude.exe");
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("a name"),
    AgentProvider::ClaudeCodeCli,
    program.clone(),
    Timestamp::now(),
  )
  .expect("an absolute program");
  let mut db = ProjectDb::open(&kontur.repo_root).expect("the project's data");
  db.create_connection(&connection).expect("stored");
  let id = connection.id();
  let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
  all_projects(&mut window);
  window.until("the warning", |window| shows(window, CLI));
  (window, stand_ins, id, program)
}

fn program_of(window: &Window, id: ConnectionId) -> PathBuf {
  ProjectDb::open(&window.data.dir.path().join("repo-kontur"))
    .expect("the project's data")
    .connection(id)
    .expect("read")
    .expect("stored")
    .executable()
    .expect("a program")
    .to_path_buf()
}

/// Lets what a press started finish, for what must not change.
fn settle(window: &mut Window) {
  for _ in 0..40 {
    window.tree.tick_futures();
    window.pass();
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
}

/// The first start, with the words, ids and measures of the frame's callout and fix row.
#[test]
fn all_projects_agent_cli_shows_on_the_welcome_as_drawn() {
  let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  assert!(shows(&mut window, "welcome-introduction"), "the welcome stays");
  assert_eq!(
    window.text("start-cli-callout-title"),
    "claude isn't found on this computer"
  );
  assert_eq!(
    window.text("start-cli-callout-body"),
    "Orchester looked in the folders on your PATH and can't find claude. Local projects open; agents can't start until it's found."
  );
  assert_eq!(window.text("start-cli-choose"), "Choose the claude file…");
  assert_eq!(window.text("start-cli-check"), "Check again");
  assert_eq!(
    window.text("start-cli-searched"),
    "There are no folders on PATH to look in."
  );

  let folder = TempDir::new().expect("a folder");
  stand_ins.search(search_in(folder.path()), SearchBasis::Path);
  window.press("start-cli-check");
  let shown = format!("Looked in {}", folder.path().display());
  window.until("the folders searched", |window| {
    window.text("start-cli-searched") == shown
  });

  let (callout, fix) = (bounds(&mut window, CALLOUT), bounds(&mut window, "start-cli-fix"));
  let gap = fix.y - (callout.y + callout.height);
  assert!((gap - 32.0).abs() < 0.5, "the fix row is {gap} under the callout");
  let (choose, check) = (
    bounds(&mut window, "start-cli-choose"),
    bounds(&mut window, "start-cli-check"),
  );
  assert!(
    (choose.height - 28.0).abs() < 0.5 && (check.height - 28.0).abs() < 0.5,
    "{choose:?} {check:?}"
  );
  assert!((check.x - (choose.x + choose.width) - 8.0).abs() < 0.5, "8 apart");
}

/// A line breaks between two folders, never inside one that fits on a line of its own; one wider than a whole line
/// breaks and stays inside the row.
#[test]
fn all_projects_agent_cli_breaks_lines_between_folders() {
  let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  let root = TempDir::new().expect("a folder");
  let [first, second, wide] = [("a", 40), ("b", 40), ("c", 300)].map(|(name, length)| root.path().join(name.repeat(length)));
  let path = std::env::join_paths([&first, &second, &wide]).expect("a search path");
  stand_ins.search(SearchPath::merge(Some(&path), &[], &[]), SearchBasis::Path);
  window.press("start-cli-check");
  let shown = format!("Looked in {}, {}, {}", first.display(), second.display(), wide.display());
  window.until("the folders searched", |window| {
    window.text("start-cli-searched") == shown
  });

  let line = bounds(&mut window, "start-cli-searched-0").height;
  let (one, two) = (
    bounds(&mut window, "start-cli-searched-1"),
    bounds(&mut window, "start-cli-searched-2"),
  );
  assert!((one.height - line).abs() < 0.5, "{one:?} is one line of {line}");
  assert!((two.height - line).abs() < 0.5, "{two:?} is one line of {line}");
  assert!(two.y >= one.y + one.height - 0.5, "the two folders do not fit on one line");

  let (searched, wide) = (
    bounds(&mut window, "start-cli-searched"),
    bounds(&mut window, "start-cli-searched-3"),
  );
  assert!(wide.height > line * 1.5, "{wide:?} breaks");
  assert!(
    wide.x + wide.width <= searched.x + searched.width + 0.5,
    "{wide:?} stays inside {searched:?}"
  );
}

/// The frame's own words, reached on any system through the search's basis.
#[test]
fn all_projects_agent_cli_says_the_frames_words_on_macos() {
  let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  let shell = "zsh".to_owned();
  stand_ins.search(SearchPath::default(), SearchBasis::LoginShell { shell });
  window.press("start-cli-check");
  window.until("the macOS title", |window| {
    window.text("start-cli-callout-title") == "claude isn't found on this Mac"
  });
  assert_eq!(
    window.text("start-cli-callout-body"),
    "Apps started from Finder don't get your terminal's PATH. Orchester read it from your login shell (zsh) and still can't find claude. Local projects open; agents can't start until it's found."
  );
  stand_ins.search(SearchPath::default(), SearchBasis::UsualFolders);
  window.press("start-cli-check");
  window.until("what it says without a read of the shell", |window| {
    window
      .text("start-cli-callout-body")
      .starts_with("Orchester looked on your PATH and in the usual install folders")
  });
  assert_eq!(window.text("start-cli-callout-title"), "claude isn't found on this Mac");
}

#[test]
fn all_projects_agent_cli_sits_above_the_list_and_a_row_still_opens() {
  let (mut window, _stand_ins, ..) = kontur_without_claude();
  let (tabs, callout, list) = (
    bounds(&mut window, "all-projects-tabs"),
    bounds(&mut window, CALLOUT),
    bounds(&mut window, "project-list"),
  );
  assert!(
    tabs.y < callout.y && callout.y < list.y,
    "{tabs:?} {callout:?} {list:?}"
  );
  window.press(&row(project_id()));
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Overview(OverviewTab::default()))
  );
}

#[test]
fn all_projects_agent_cli_chosen_file_fixes_the_connection_and_the_warning_goes() {
  let (mut window, stand_ins, id, _) = kontur_without_claude();
  let folder = TempDir::new().expect("a folder");
  let chosen = write_claude(folder.path());
  stand_ins.choose(Some(chosen.clone()));
  window.press("start-cli-choose");
  window.until("the warning to go", |window| !shows(window, CLI));
  assert_eq!(program_of(&window, id), chosen);
  window.press(&row(project_id()));
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Overview(OverviewTab::default()))
  );
}

#[test]
fn all_projects_agent_cli_first_start_goes_once_a_file_is_chosen() {
  let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  let folder = TempDir::new().expect("a folder");
  stand_ins.choose(Some(write_claude(folder.path())));
  window.press("start-cli-choose");
  window.until("the warning to go", |window| !shows(window, CLI));
  assert!(shows(&mut window, "welcome-introduction"), "the welcome stays");
  assert!(!shows(&mut window, "welcome-scroll"), "and needs no scroll without it");
}

#[test]
fn all_projects_agent_cli_wrong_file_says_why_and_changes_nothing() {
  let (mut window, stand_ins, id, program) = kontur_without_claude();
  let folder = TempDir::new().expect("a folder");
  let wrong = write_program(folder.path(), "gh", "gh version 2.62.0");
  stand_ins.choose(Some(wrong.clone()));
  window.press("start-cli-choose");
  window.until("the refusal", |window| shows(window, ERROR));
  assert_eq!(
    window.text(ERROR),
    format!(
      "{} did not answer as claude. Choose the claude program file.",
      wrong.display()
    )
  );
  assert!(shows(&mut window, CLI), "the warning stays");
  assert_eq!(program_of(&window, id), program, "nothing changed");
}

#[test]
fn all_projects_agent_cli_closed_picker_changes_nothing() {
  let (mut window, stand_ins, id, program) = kontur_without_claude();
  stand_ins.choose(None);
  window.press("start-cli-choose");
  settle(&mut window);
  assert!(shows(&mut window, CLI), "the warning stays");
  assert!(!shows(&mut window, ERROR), "closing the picker is no error");
  assert_eq!(program_of(&window, id), program);
}

#[test]
fn all_projects_agent_cli_check_again_finds_a_copy_added_later() {
  let (mut window, stand_ins, ..) = kontur_without_claude();
  let folder = TempDir::new().expect("a folder");
  stand_ins.search(search_in(folder.path()), SearchBasis::Path);
  assert_eq!(
    *stand_ins.shell_reads.lock(),
    0,
    "only Check again reads the shell again"
  );
  window.press("start-cli-check");
  let shown = format!("Looked in {}", folder.path().display());
  window.until("the search again", |window| window.text("start-cli-searched") == shown);
  assert_eq!(*stand_ins.shell_reads.lock(), 1);
  write_claude(folder.path());
  window.press("start-cli-check");
  window.until("the warning to go", |window| !shows(window, CLI));
  assert_eq!(*stand_ins.shell_reads.lock(), 2);
}

#[test]
fn all_projects_agent_cli_is_not_on_usage() {
  let (mut window, _stand_ins, ..) = kontur_without_claude();
  window.press("all-projects-tab-usage");
  window.until("the Usage tab", |window| shows(window, "all-projects-top"));
  assert!(!shows(&mut window, CLI));
  window.press("all-projects-tab-projects");
  window.until("the warning again", |window| shows(window, CLI));
}

#[test]
fn all_projects_agent_cli_is_not_on_a_list_that_cannot_be_read() {
  let data = Data::new();
  let launch = data.launch();
  let database = data.database();
  rusqlite::Connection::open(&database)
    .expect("the application database")
    .execute("DROP TABLE projects", [])
    .expect("the list can't be read");
  let (mut window, _stand_ins) = Window::launched_with_stand_ins(data, launch);
  window.navigation().go(Route::AllProjects);
  window.until("the list that cannot be read", |window| {
    shows(window, "all-projects-failed")
  });
  settle(&mut window);
  assert!(!shows(&mut window, CLI));
}

/// Removing the last project is no first launch, but nothing names a CLI any more: the rule of a first start holds.
#[test]
fn all_projects_agent_cli_shows_on_an_emptied_list_as_at_a_first_start() {
  let (mut window, stand_ins, ..) = kontur_without_claude();
  window.press(&format!("project-remove-{}", project_id()));
  window.press("remove-project-confirm");
  window.until("the empty list", |window| shows(window, "all-projects-empty"));
  window.until("the warning", |window| shows(window, CLI));
  let (callout, empty) = (bounds(&mut window, CALLOUT), bounds(&mut window, "all-projects-empty"));
  assert!(callout.y < empty.y, "{callout:?} {empty:?}");
  let folder = TempDir::new().expect("a folder");
  stand_ins.choose(Some(write_claude(folder.path())));
  window.press("start-cli-choose");
  window.until("the warning to go", |window| !shows(window, CLI));
  assert!(shows(&mut window, "all-projects-empty"), "the empty list stays");
}

/// The welcome with the warning does not fit the window's smallest size, so only then it scrolls.
#[test]
fn all_projects_agent_cli_welcome_scrolls_at_the_smallest_window() {
  let (mut window, _stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  window.tree.resize(960, 640);
  window.pass();
  let (scroll, fix) = (
    bounds(&mut window, "welcome-scroll"),
    bounds(&mut window, "start-cli-fix"),
  );
  assert!(
    fix.y + fix.height > scroll.y + scroll.height,
    "the fix row is further down than the window shows: {fix:?} {scroll:?}"
  );
}

/// In a window with room to spare the welcome and the warning stay centred above the footer, as the welcome alone does.
#[test]
fn all_projects_agent_cli_welcome_stays_centred_in_a_large_window() {
  let (mut window, _stand_ins) = Window::with_stand_ins(Data::new());
  window.until("the warning", |window| shows(window, CLI));
  window.tree.resize(1440, 900);
  window.pass();
  let (screen, introduction, steps, fix) = (
    bounds(&mut window, "screen-all_projects"),
    bounds(&mut window, "welcome-introduction"),
    bounds(&mut window, "welcome-steps"),
    bounds(&mut window, "start-cli-fix"),
  );
  let above = introduction.y.min(steps.y) - screen.y;
  let below = screen.y + screen.height - FOOTER_HEIGHT - (fix.y + fix.height);
  assert!(
    (above - below).abs() < 1.0,
    "{above} above and {below} below: {screen:?} {introduction:?} {steps:?} {fix:?}"
  );
}
