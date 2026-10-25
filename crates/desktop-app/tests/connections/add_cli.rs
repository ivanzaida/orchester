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
  assert_eq!(
    seeded.text("connection-check-detail"),
    format!("Codex CLI {VERSION} · signed in")
  );
  // On an API provider the CLI's own sign-in is not used, and the check says so (`virfc`).
  seeded.click("connection-runs-provider");
  assert_eq!(
    seeded.text("connection-check-detail"),
    format!("Codex CLI {VERSION} · runs on the API provider; its own sign-in is not used")
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
    seeded.text("connection-check-detail") == format!("Codex CLI {VERSION} · signed in")
  });
  assert_eq!(value(&mut seeded, "connection-name"), "Codex · Kontur 2");
  seeded.until("the selected provider settles", |s| !s.window.tree.needs_redraw());
  let folder = env::split_paths(&env::var_os("PATH").expect("the child's search path"))
    .next()
    .expect("the stand-in directory");
  write_codex(&folder, "999.0.1");
  seeded.click("connection-recheck");
  seeded.until("the refreshed program's actual version", |seeded| {
    seeded.text("connection-check-detail") == "Codex CLI 999.0.1 · signed in"
  });
  seeded.until("Recheck leaves the window settled", |s| !s.window.tree.needs_redraw());
  seeded.set("connection-name", "Claude · Kontur");
  assert!(seeded.text("connection-form-reason").contains("already has this name"));
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

fn value(seeded: &mut Seeded, id: &str) -> String {
  seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
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
