//! Add connection for a CLI: the program offered is the copy found on the search path, checked at once, and
//! the connection is saved with that check. The search path is the process's `PATH`, which a test cannot
//! change for itself alone, so the test runs itself again in a child process whose `PATH` starts with a
//! folder holding a stand-in Codex: it answers `--version` and `login status` as Codex does, with a version
//! newer than any real one, so it is the copy offered first.

use std::{
  env,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_app::routes::NewConnection;
use orchester_core::{
  connection::{AgentConnection, ConnectionStatus},
  store::ConnectionStore,
};

use super::harness::Seeded;

/// Set in the child that runs the test with the stand-in on its `PATH`.
const CHILD: &str = "ORCHESTER_TEST_CONNECTIONS_FAKE_CODEX";
const VERSION: &str = "999.0.0";

#[test]
fn connections_cli_is_added_from_the_search_path_and_checked() {
  if env::var_os(CHILD).is_some() {
    add_codex();
    return;
  }
  let folder = tempfile::tempdir().expect("a temporary folder");
  write_codex(folder.path(), VERSION);
  let mut path = vec![folder.path().to_path_buf()];
  path.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
  let status = Command::new(env::current_exe().expect("the test binary"))
    .args([
      "--exact",
      "add_cli::connections_cli_is_added_from_the_search_path_and_checked",
      "--nocapture",
    ])
    .env("PATH", env::join_paths(path).expect("a search path"))
    .env(CHILD, "1")
    .status()
    .expect("the test runs again");
  assert!(status.success(), "the test with the stand-in on the PATH failed");
}

fn add_codex() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  seeded.until("the check", |seeded| {
    seeded.exists("connection-check") && seeded.text("connection-check").contains(VERSION)
  });
  // What checking found is one line under the program (`Note` `mvJ49`).
  assert_eq!(
    seeded.text("connection-check-detail"),
    format!("codex {VERSION} · signed in")
  );
  assert!(seeded.text("connection-program-description").contains("on the PATH"));
  // On an API provider the CLI's own sign-in is not used, and the check says so (`virfc`).
  seeded.click("connection-runs-provider");
  assert_eq!(
    seeded.text("connection-check-detail"),
    format!("codex {VERSION} · runs on the API provider; its own sign-in is not used")
  );
  seeded.click("connection-runs-sign-in");
  seeded.until("the completed check leaves the window settled", |s| {
    !s.window.tree.needs_redraw()
  });
  // The suggested name is numbered: the project has a Codex · Kontur.
  assert_eq!(value(&mut seeded, "connection-name"), "Codex · Kontur 2");
  seeded.click("connection-kind-claude");
  seeded.click("connection-kind-codex");
  seeded.until("the current provider's check", |seeded| {
    seeded.text("connection-check-detail") == format!("codex {VERSION} · signed in")
  });
  assert_eq!(value(&mut seeded, "connection-name"), "Codex · Kontur 2");
  seeded.until("the selected provider settles", |s| !s.window.tree.needs_redraw());
  let folder = env::split_paths(&env::var_os("PATH").expect("the child's search path"))
    .next()
    .expect("the stand-in directory");
  write_codex(&folder, "999.0.1");
  seeded.click("connection-recheck");
  seeded.until("the refreshed program's actual version", |seeded| {
    seeded.text("connection-check-detail") == "codex 999.0.1 · signed in"
  });
  seeded.until("Recheck leaves the window settled", |s| !s.window.tree.needs_redraw());
  seeded.set("connection-name", "Claude · Kontur");
  assert!(seeded.text(Seeded::REASON).contains("already has this name"));
  seeded.set("connection-name", "Codex · local");
  seeded.click("connection-save");
  seeded.until("the saved connection", |seeded| seeded.exists("connection-inspector"));
  let saved: AgentConnection = seeded
    .db()
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == "Codex · local")
    .map(|connection| AgentConnection::try_from(connection).expect("an agent connection"))
    .expect("the connection is stored");
  assert_eq!(saved.status_with(&[]), ConnectionStatus::Connected);
  let program = saved.executable().expect("a program").to_path_buf();
  assert!(
    program.starts_with(
      env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .next()
        .expect("a folder")
    )
  );
  assert_eq!(seeded.selected(), Some(saved.id()));
}

/// The file chosen on the start page is offered as the program, and says that is where it came from. In a child whose
/// `PATH` leaves out every folder holding a real CLI, so the file is the only copy.
#[test]
fn chosen_cli_file_is_offered_in_add_connection() {
  if env::var_os(CHILD).is_some() {
    offer_chosen_claude();
    return;
  }
  let inherited = env::var_os("PATH").unwrap_or_default();
  let path = env::split_paths(&inherited).filter(|folder| !holds_a_cli(folder));
  let status = Command::new(env::current_exe().expect("the test binary"))
    .args([
      "--exact",
      "add_cli::chosen_cli_file_is_offered_in_add_connection",
      "--nocapture",
    ])
    .env("PATH", env::join_paths(path).expect("a search path"))
    .env(CHILD, "1")
    .status()
    .expect("the test runs again");
  assert!(status.success(), "the test without a real CLI on the PATH failed");
}

fn offer_chosen_claude() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let chosen = write_claude(folder.path());
  let mut seeded = Seeded::new("connections");
  seeded
    .window
    .launch
    .workspace()
    .expect("the session started")
    .set_ui_state(
      "orchester-app.agentProgram.claude",
      chosen.to_str().expect("a plain path"),
    )
    .expect("kept");
  seeded.add(NewConnection::AgentProvider);
  seeded.click("connection-kind-claude");
  seeded.until("the check", |seeded| {
    seeded.exists("connection-check") && seeded.text("connection-check").contains("2.1.296")
  });
  assert_eq!(value(&mut seeded, "connection-program"), chosen.display().to_string());
  assert_eq!(
    seeded.text("connection-program-description"),
    "The file you chose on the start page"
  );
}

/// Whether `folder` holds a real `claude` or `codex`.
fn holds_a_cli(folder: &Path) -> bool {
  ["claude", "codex"].iter().any(|program| {
    ["", ".exe", ".cmd", ".bat", ".ps1"]
      .iter()
      .any(|extension| folder.join(format!("{program}{extension}")).is_file())
  })
}

fn value(seeded: &mut Seeded, id: &str) -> String {
  seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

/// A stand-in Claude Code in `folder`.
#[cfg(windows)]
fn write_claude(folder: &Path) -> PathBuf {
  let program = folder.join("claude.cmd");
  let script = "@echo off\r\nif \"%1\"==\"--version\" goto version\r\nif \"%1\"==\"auth\" goto status\r\nexit /b 1\r\n:version\r\necho 2.1.296 ^(Claude Code^)\r\nexit /b 0\r\n:status\r\necho {\"loggedIn\":true}\r\nexit /b 0\r\n";
  std::fs::write(&program, script).expect("the stand-in is written");
  program
}

/// A stand-in Claude Code in `folder`.
#[cfg(not(windows))]
fn write_claude(folder: &Path) -> PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let program = folder.join("claude");
  let script = "#!/bin/sh\ncase \"$1\" in\n  --version) echo '2.1.296 (Claude Code)' ;;\n  auth) echo '{\"loggedIn\":true}' ;;\n  *) exit 1 ;;\nesac\n";
  std::fs::write(&program, script).expect("the stand-in is written");
  std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
  program
}

/// A stand-in Codex in `folder`.
#[cfg(windows)]
pub(super) fn write_codex(folder: &Path, version: &str) -> PathBuf {
  let program = folder.join("codex.cmd");
  let script = format!(
    "@echo off\r\nif \"%1\"==\"--version\" (echo codex-cli {version}& exit /b 0)\r\nif \"%1\"==\"login\" (echo Logged in using ChatGPT& exit /b 0)\r\nexit /b 1\r\n"
  );
  std::fs::write(&program, script).expect("the stand-in is written");
  program
}

/// A stand-in Codex in `folder`.
#[cfg(not(windows))]
pub(super) fn write_codex(folder: &Path, version: &str) -> PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let program = folder.join("codex");
  let script = format!(
    "#!/bin/sh\ncase \"$1\" in\n  --version) echo \"codex-cli {version}\" ;;\n  login) echo \"Logged in using ChatGPT\" ;;\n  *) exit 1 ;;\nesac\n"
  );
  std::fs::write(&program, script).expect("the stand-in is written");
  std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
  program
}

/// A stand-in Codex 999.0.0 in `folder` that creates `asked` when asked its version, then takes two seconds to answer.
#[cfg(windows)]
pub(super) fn write_slow_codex(folder: &Path, asked: &Path) -> PathBuf {
  let program = folder.join("codex.cmd");
  let asked = asked.display();
  let script = format!(
    "@echo off\r\nif \"%1\"==\"--version\" (echo.>\"{asked}\"& ping -n 3 127.0.0.1 >nul& echo codex-cli 999.0.0& exit /b 0)\r\nif \"%1\"==\"login\" (echo Logged in using ChatGPT& exit /b 0)\r\nexit /b 1\r\n"
  );
  std::fs::write(&program, script).expect("the stand-in is written");
  program
}

/// A stand-in Codex 999.0.0 in `folder` that creates `asked` when asked its version, then takes two seconds to answer.
#[cfg(not(windows))]
pub(super) fn write_slow_codex(folder: &Path, asked: &Path) -> PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let program = folder.join("codex");
  let asked = asked.display();
  let script = format!(
    "#!/bin/sh\ncase \"$1\" in\n  --version) touch \"{asked}\"; sleep 2; echo \"codex-cli 999.0.0\" ;;\n  login) echo \"Logged in using ChatGPT\" ;;\n  *) exit 1 ;;\nesac\n"
  );
  std::fs::write(&program, script).expect("the stand-in is written");
  std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
  program
}
