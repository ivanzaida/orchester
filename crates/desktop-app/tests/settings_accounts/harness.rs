//! Settings › Accounts on a window of its own, with stand-in CLIs. Each test runs itself again in a child process
//! whose `PATH` starts with a folder holding a stand-in `claude` and `codex`: they answer `--version` and their
//! status command, signed in when the folder their variable names holds a `signed-in` file. The child's
//! `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, home folder and folder for new accounts are throwaway folders too, so no test
//! asks a real CLI or reads a real sign-in.

use std::{env, fs, path::Path, process::Command};

use lurq::app::events::MouseButton;
use orchester_app::routes::{AccountsPanel, AccountsView, Route};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_store::AppDb;

#[path = "../support/mod.rs"]
mod support;

pub use support::Window;

/// Set in the child, to the folder that holds the stand-ins and the throwaway folders.
const CHILD: &str = "ORCHESTER_TEST_ACCOUNTS_ROOT";

/// Runs `body` in a child process set up with the stand-ins; `name` is the test's path in this binary.
pub fn in_child(name: &str, body: impl FnOnce(&Path)) {
  if let Some(root) = env::var_os(CHILD) {
    body(Path::new(&root));
    return;
  }
  let root = tempfile::tempdir().expect("a temporary folder");
  let folder = |name: &str| {
    let folder = root.path().join(name);
    fs::create_dir_all(&folder).expect("a throwaway folder");
    folder
  };
  let bin = folder("bin");
  write_stand_ins(&bin);
  let mut path = vec![bin];
  path.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
  let home = folder("home");
  let status = Command::new(env::current_exe().expect("the test binary"))
    .args(["--exact", name, "--nocapture"])
    .env("PATH", env::join_paths(path).expect("a search path"))
    .env("CLAUDE_CONFIG_DIR", folder("home/.claude"))
    .env("CODEX_HOME", folder("home/.codex"))
    .env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, &home)
    .env("ORCHESTER_ACCOUNTS_DIR", folder("accounts"))
    .env(CHILD, root.path())
    .status()
    .expect("the test runs again");
  assert!(status.success(), "{name} with the stand-in CLIs failed");
}

/// Marks `folder` signed in for the stand-ins.
pub fn sign_in(folder: &Path) {
  fs::write(folder.join("signed-in"), "").expect("the stand-in's sign-in is written");
}

/// Adds an account to the window's application database in `folder`, which is made, as Orchester created it.
pub fn add_account(window: &Window, cli: AccountCli, name: &str, folder: &Path) -> AgentAccount {
  fs::create_dir_all(folder).expect("the account's folder");
  let account = AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name).expect("a name"),
    AccountFolder::Created(folder.to_path_buf()),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&window.data.database())
    .expect("the application database")
    .add_agent_account(&account)
    .expect("added");
  account
}

/// The pages of Settings › Accounts: the list, an account's, *Add account* and its sign-in step.
const PAGES: [&str; 4] = [
  "settings-accounts",
  "account-page",
  "account-add-page",
  "account-sign-in-page",
];

/// Opens Settings › Accounts at `panel` of `account`.
pub fn go(window: &mut Window, account: Option<AgentAccountId>, panel: AccountsPanel) {
  window.navigation().go(Route::Accounts(AccountsView { account, panel }));
  window.until("the accounts", |window| {
    PAGES.iter().any(|page| window.texts(page).is_some())
  });
}

pub fn click(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

pub fn set(window: &mut Window, id: &str, value: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .set_value(value);
  window.pass();
}

/// The texts of `id` joined with spaces; empty when it is not shown.
pub fn joined(window: &mut Window, id: &str) -> String {
  window.texts(id).map(|texts| texts.join(" ")).unwrap_or_default()
}

pub fn shown(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

#[cfg(windows)]
fn write_stand_ins(folder: &Path) {
  let claude = "@echo off\r\n\
    if \"%1\"==\"--version\" goto version\r\n\
    if \"%1\"==\"auth\" if \"%2\"==\"status\" goto status\r\n\
    exit /b 1\r\n\
    :version\r\n\
    echo 2.1.296 ^(Claude Code^)\r\n\
    exit /b 0\r\n\
    :status\r\n\
    if exist \"%CLAUDE_CONFIG_DIR%\\signed-in\" goto in\r\n\
    echo {\"loggedIn\":false,\"authMethod\":\"none\"}\r\n\
    exit /b 1\r\n\
    :in\r\n\
    echo {\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"email\":\"ana@example.com\",\"subscriptionType\":\"max\"}\r\n\
    exit /b 0\r\n";
  let codex = "@echo off\r\n\
    if \"%1\"==\"--version\" goto version\r\n\
    if \"%1\"==\"login\" goto login\r\n\
    exit /b 1\r\n\
    :version\r\n\
    echo codex-cli 0.159.2\r\n\
    exit /b 0\r\n\
    :login\r\n\
    if exist \"%CODEX_HOME%\\signed-in\" goto in\r\n\
    echo Not logged in\r\n\
    exit /b 1\r\n\
    :in\r\n\
    echo Logged in using ChatGPT\r\n\
    exit /b 0\r\n";
  fs::write(folder.join("claude.cmd"), claude).expect("the stand-in claude");
  fs::write(folder.join("codex.cmd"), codex).expect("the stand-in codex");
}

#[cfg(not(windows))]
fn write_stand_ins(folder: &Path) {
  use std::os::unix::fs::PermissionsExt;
  let claude = "#!/bin/sh\n\
    case \"$1 $2\" in\n\
      --version*) echo '2.1.296 (Claude Code)' ;;\n\
      'auth status') if [ -e \"$CLAUDE_CONFIG_DIR/signed-in\" ]; then \
        echo '{\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"email\":\"ana@example.com\",\"subscriptionType\":\"max\"}'; \
        else echo '{\"loggedIn\":false,\"authMethod\":\"none\"}'; exit 1; fi ;;\n\
      *) exit 1 ;;\n\
    esac\n";
  let codex = "#!/bin/sh\n\
    case \"$1\" in\n\
      --version) echo 'codex-cli 0.159.2' ;;\n\
      login) if [ -e \"$CODEX_HOME/signed-in\" ]; then echo 'Logged in using ChatGPT'; \
        else echo 'Not logged in'; exit 1; fi ;;\n\
      *) exit 1 ;;\n\
    esac\n";
  for (name, script) in [("claude", claude), ("codex", codex)] {
    let program = folder.join(name);
    fs::write(&program, script).expect("a stand-in");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
  }
}
