//! Settings › Accounts (`docs/design/agent-accounts.md`, layer `hgOl2`): the accounts listed by CLI with their states
//! once their CLIs were asked, the shown one's details, adding one in place with a folder the person already uses,
//! the sign-in step, renaming, and removing with its connections moved. Stand-in CLIs answer (see `harness`).

mod harness;

use std::{env, fs};

use harness::{Window, add_account, click, go, in_child, joined, set, shown, sign_in};
use orchester_app::routes::AccountsPanel;
use orchester_core::{
  account::{AccountCli, AccountState},
  connection::{AgentConnection, AgentProvider, ConnectionAccount, invocation::Platform},
  id::ConnectionId,
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

fn home() -> std::path::PathBuf {
  env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
    .map(Into::into)
    .expect("the child's home folder")
}

#[test]
fn settings_accounts_lists_accounts_by_cli_with_what_their_clis_say() {
  in_child(
    "settings_accounts_lists_accounts_by_cli_with_what_their_clis_say",
    |root| {
      let mut window = Window::open();
      let work = add_account(
        &window,
        AccountCli::Codex,
        "Work",
        &root.join("accounts").join("codex-work"),
      );
      sign_in(work.folder().path().expect("a folder"));
      go(&mut window, Some(work.id()), AccountsPanel::Account);
      let row = format!("account-row-{}", work.id());
      window.until("Work's check", |window| joined(window, &row).contains("Signed in"));
      assert!(joined(&mut window, &row).contains("Work"));
      assert!(joined(&mut window, "accounts-group-claude_code").contains("1 account"));
      assert!(joined(&mut window, "accounts-group-codex").contains("2 accounts"));
      let default = format!("account-row-{}", AccountCli::Codex.default_account());
      window.until("Default's check", |window| {
        joined(window, &default).contains("Not signed in")
      });
      assert!(joined(&mut window, &default).contains("The CLI's own folder · sign in to use it"));
      assert!(joined(&mut window, "settings-tab-accounts").contains("Accounts 3"));
      assert!(joined(&mut window, "account-title").contains("Work"));
      assert!(joined(&mut window, "account-fact-checked-with").contains("codex 0.159.2 on the PATH"));
      assert!(joined(&mut window, "account-fact-method").contains("ChatGPT"));
      assert!(joined(&mut window, "account-runs-get").contains("CODEX_HOME"));
      assert!(joined(&mut window, "account-no-token").contains("codex login status"));
      assert!(shown(&mut window, "settings-tab-general") && shown(&mut window, "settings-tab-servers"));
    },
  );
}

#[test]
fn settings_accounts_shows_why_an_account_does_not_run_and_the_login_to_copy() {
  in_child(
    "settings_accounts_shows_why_an_account_does_not_run_and_the_login_to_copy",
    |_| {
      let mut window = Window::open();
      let claude = AccountCli::ClaudeCode.default_account();
      go(&mut window, Some(claude), AccountsPanel::Account);
      window.until("the state's sentence", |window| {
        joined(window, "account-state-sentence").contains("isn't signed in")
      });
      let command = joined(&mut window, "account-sign-in-command").replace(' ', "");
      assert!(command.contains("claudeauthlogin"), "{command}");
      let remove = joined(&mut window, "account-remove");
      assert!(remove.contains("Remove"));
      click(&mut window, "account-remove");
      assert!(
        !shown(&mut window, "account-remove-question"),
        "Default can't be removed"
      );
    },
  );
}

#[test]
fn settings_accounts_adds_a_folder_the_person_already_uses_and_refuses_one_in_a_repository() {
  in_child(
    "settings_accounts_adds_a_folder_the_person_already_uses_and_refuses_one_in_a_repository",
    |root| {
      let mut window = Window::open();
      go(&mut window, None, AccountsPanel::Add);
      click(&mut window, "account-add-cli-codex");
      set(&mut window, "account-add-name", "Client");
      assert!(joined(&mut window, "account-row-adding").contains("Client"));
      click(&mut window, "account-add-folder-existing");
      let repository = root.join("repo");
      fs::create_dir_all(repository.join(".git")).expect("a repository");
      set(
        &mut window,
        "account-add-path",
        &repository.join(".codex-ci").display().to_string(),
      );
      window.until("the folder's refusal", |window| {
        joined(window, "account-add-folder-error").contains("inside the repository")
      });
      let brought = home().join(".codex-client");
      fs::create_dir_all(&brought).expect("the person's folder");
      sign_in(&brought);
      set(&mut window, "account-add-path", &brought.display().to_string());
      window.until("the folder taken", |window| !shown(window, "account-add-folder-error"));
      click(&mut window, "account-add-save");
      let workspace = window.launch.workspace().expect("the session").clone();
      window.until("the new account's details", |window| {
        joined(window, "account-title").contains("Client")
          && joined(window, "account-title-state").contains("Signed in")
      });
      let added = workspace
        .agent_accounts()
        .expect("listed")
        .into_iter()
        .find(|account| account.name().is_some_and(|name| name.as_str() == "Client"))
        .expect("Client is stored");
      assert_eq!(added.folder().path(), Some(brought.as_path()));
      assert_eq!(added.state(Platform::current()), AccountState::SignedIn);
    },
  );
}

#[test]
fn settings_accounts_sign_in_step_shows_the_login_for_the_accounts_folder_and_checks_again() {
  in_child(
    "settings_accounts_sign_in_step_shows_the_login_for_the_accounts_folder_and_checks_again",
    |root| {
      let mut window = Window::open();
      let folder = root.join("accounts").join("codex-freelance");
      let freelance = add_account(&window, AccountCli::Codex, "Freelance", &folder);
      go(&mut window, Some(freelance.id()), AccountsPanel::SignIn);
      window.until("the sign-in step", |window| shown(window, "account-sign-in-terminal"));
      let command = joined(&mut window, "account-sign-in-own").replace(' ', "");
      assert!(
        command.contains("CODEX_HOME") && command.contains("codexlogin"),
        "{command}"
      );
      assert!(joined(&mut window, "account-add-step-1").contains("Freelance · folder created"));
      window.until("the first check", |window| shown(window, "account-sign-in-check"));
      sign_in(&folder);
      window.until("Check again to be pressable", |window| {
        !joined(window, &format!("account-row-{}", freelance.id())).contains("Checking")
      });
      click(&mut window, "account-sign-in-check");
      window.until("the signed-in outcome", |window| shown(window, "account-sign-in-done"));
      assert!(joined(&mut window, "account-sign-in-done").contains("Signed in"));
    },
  );
}

#[test]
fn settings_accounts_renames_in_place() {
  in_child("settings_accounts_renames_in_place", |root| {
    let mut window = Window::open();
    let work = add_account(
      &window,
      AccountCli::Codex,
      "Work",
      &root.join("accounts").join("codex-work"),
    );
    go(&mut window, Some(work.id()), AccountsPanel::Rename);
    window.until("the name field", |window| shown(window, "account-rename-field"));
    set(&mut window, "account-rename-field", "Kontur team");
    click(&mut window, "account-rename-save");
    window.until("the new name", |window| {
      joined(window, "account-title").contains("Kontur team")
    });
    let stored = window
      .launch
      .workspace()
      .expect("the session")
      .agent_account(work.id())
      .expect("read")
      .expect("stored");
    assert_eq!(stored.name(), Some(&Name::new("Kontur team").expect("a name")));
    assert_eq!(stored.folder(), work.folder(), "the folder keeps its path");
  });
}

#[test]
fn settings_accounts_removes_an_account_and_moves_its_connections() {
  in_child(
    "settings_accounts_removes_an_account_and_moves_its_connections",
    |root| {
      let mut window = Window::open();
      let work = add_account(
        &window,
        AccountCli::Codex,
        "Work",
        &root.join("accounts").join("codex-work"),
      );
      let repo = window.data.repo("repo-kontur");
      let mut connection = AgentConnection::new(
        ConnectionId::generate(),
        Name::new("Codex · Work").expect("a name"),
        AgentProvider::CodexCli,
        root.join("bin").join("codex.cmd"),
        Timestamp::now(),
      )
      .expect("a connection");
      connection
        .set_account(ConnectionAccount::of(&work))
        .expect("an agent CLI");
      ProjectDb::open(&repo)
        .expect("the project")
        .create_connection(&connection)
        .expect("created");
      go(&mut window, Some(work.id()), AccountsPanel::UsedBy);
      window.until("what uses Work", |window| {
        joined(window, "account-used-by").contains("Codex · Work")
      });
      assert!(joined(&mut window, "account-tab-used-by").contains("Used by 1"));
      click(&mut window, "account-remove");
      window.until("the question", |window| shown(window, "account-remove-question"));
      assert!(joined(&mut window, "account-remove-question").contains("Remove Work from Orchester?"));
      click(&mut window, "account-remove-confirm");
      let workspace = window.launch.workspace().expect("the session").clone();
      window.until("Work removed", |_| {
        workspace
          .agent_accounts()
          .expect("listed")
          .iter()
          .all(|account| account.id() != work.id())
      });
      let moved = ProjectDb::open(&repo)
        .expect("the project")
        .connection(connection.id())
        .expect("read")
        .expect("stored");
      assert_eq!(moved.account(), &ConnectionAccount::Default);
      assert!(
        work.folder().path().expect("a folder").is_dir(),
        "the folder stays unless ticked"
      );
    },
  );
}
