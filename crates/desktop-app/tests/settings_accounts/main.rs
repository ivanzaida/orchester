//! Settings › Accounts (`03 · Accounts` `t8i6m`, `03b · Account` `xN1Jy`, `03c · Add account` `XAm8i`, states on
//! `poD7g`): the accounts listed by CLI with their states once their CLIs were asked, an account's page, adding one
//! with a folder the person already uses, the sign-in step, renaming in place, and removing in place with its
//! connections moved. Stand-in CLIs answer (see `harness`).

mod harness;
mod history;

use std::{env, fs};

use harness::{Window, add_account, click, go, in_child, joined, set, shown, sign_in};
use orchester_app::routes::AccountsPanel;
use orchester_core::{
  account::{AccountCli, AccountState},
  connection::{AgentConnection, AgentProvider, ConnectionAccount},
  id::{AgentAccountId, ConnectionId},
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

/// An account's row in the list.
fn row(account: AgentAccountId) -> String {
  format!("account-row-{account}-row")
}

/// Opens the account page's *…* menu.
fn more(window: &mut Window) {
  window.until("the page's head", |window| shown(window, "account-more"));
  click(window, "account-more");
  window.until("its menu", |window| shown(window, "account-more-menu"));
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
      go(&mut window, None, AccountsPanel::Account);
      window.until("Work's check", |window| {
        joined(window, &row(work.id())).contains("Signed in")
      });
      assert!(joined(&mut window, &row(work.id())).contains("Work"));
      assert!(joined(&mut window, "accounts-group-claude_code").contains("1 account"));
      assert!(joined(&mut window, "accounts-group-codex").contains("2 accounts"));
      let default = AccountCli::Codex.default_account();
      window.until("Default's check", |window| {
        joined(window, &row(default)).contains("Not signed in")
      });
      assert!(joined(&mut window, &row(default)).contains("The CLI's own folder · sign in to use it"));
      let top = |window: &mut Window, account: AgentAccountId| {
        window
          .tree
          .get_element_by_id_mut(&row(account))
          .and_then(|element| element.bounds())
          .expect("the row is laid out")
          .y
      };
      assert!(
        top(&mut window, work.id()) < top(&mut window, default),
        "Default sorts last"
      );
      assert!(joined(&mut window, "settings-tab-accounts").contains("Accounts 3"));
      click(&mut window, &format!("account-row-{}-open", work.id()));
      window.until("Work's page", |window| {
        joined(window, "account-head-title").contains("Codex · Work")
      });
      assert!(joined(&mut window, "account-fact-checked-with-row").contains("codex 0.159.2 on the PATH"));
      // A long folder path keeps to its width, so the label and its hint keep theirs (`c0L3F` in `xN1Jy`).
      let mut bounds = |id: &str| {
        window
          .tree
          .get_element_by_id_mut(id)
          .and_then(|element| element.bounds())
          .unwrap_or_else(|| panic!("{id} is laid out"))
      };
      let (folder, path, hint) = (
        bounds("account-fact-folder-row"),
        bounds("account-fact-folder-path"),
        bounds("account-fact-folder-hint"),
      );
      assert!(path.width <= 325.5, "{path:?}");
      assert!(hint.height < 20.0, "the hint is one line: {hint:?}");
      assert!(folder.height < 70.0, "{folder:?}");
      assert!(joined(&mut window, "account-fact-method-row").contains("ChatGPT"));
      assert!(joined(&mut window, "account-facts-note").contains("codex login status"));
      assert!(shown(&mut window, "settings-tab-general") && shown(&mut window, "settings-tab-orchester"));
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
      // The sign-in and Check again are the notice's actions under the head (`M32Lv`), not the folder's.
      let actions = joined(&mut window, "account-state-sentence-actions");
      assert!(
        actions.contains("Sign in in a terminal") && actions.contains("Check again"),
        "{actions}"
      );
      let folder = joined(&mut window, "account-fact-folder-row");
      assert!(!folder.contains("Sign in"), "{folder}");
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
      window.until("Add account's page", |window| shown(window, "account-add-page"));
      assert!(joined(&mut window, "account-add-crumb").contains("Accounts"));
      click(&mut window, "account-add-cli-codex");
      set(&mut window, "account-add-name", "Client");
      assert!(joined(&mut window, "account-add-name-row-hint").contains("Codex · Client"));
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
      window.until("the new account's page", |window| {
        joined(window, "account-head-title").contains("Client")
          && joined(window, "account-title-state").contains("Signed in")
      });
      let added = workspace
        .agent_accounts()
        .expect("listed")
        .into_iter()
        .find(|account| account.name().is_some_and(|name| name.as_str() == "Client"))
        .expect("Client is stored");
      assert_eq!(added.folder().path(), Some(brought.as_path()));
      assert_eq!(added.state(), AccountState::SignedIn);
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
      window.until("the sign-in step", |window| shown(window, "account-sign-in"));
      let command = joined(&mut window, "account-sign-in-own").replace(' ', "");
      assert!(
        command.contains("CODEX_HOME") && command.contains("codexlogin"),
        "{command}"
      );
      assert!(joined(&mut window, "account-add-step-1").contains("Freelance · folder created"));
      let workspace = window.launch.workspace().expect("the session").clone();
      window.until("the first check", |_| {
        workspace
          .agent_account(freelance.id())
          .expect("read")
          .is_some_and(|account| account.last_check().is_some())
      });
      sign_in(&folder);
      window.until("the signed-in outcome", |window| {
        if !shown(window, "account-sign-in-done") && shown(window, "account-sign-in-check") {
          click(window, "account-sign-in-check");
        }
        shown(window, "account-sign-in-done")
      });
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
    go(&mut window, Some(work.id()), AccountsPanel::Account);
    more(&mut window);
    click(&mut window, "account-rename");
    window.until("the name field in the head", |window| {
      shown(window, "account-rename-field")
    });
    set(&mut window, "account-rename-field", "Kontur team");
    click(&mut window, "account-rename-save");
    window.until("the new name", |window| {
      joined(window, "account-head-title").contains("Kontur team")
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
fn settings_accounts_removes_an_account_in_place_and_moves_its_connections() {
  in_child(
    "settings_accounts_removes_an_account_in_place_and_moves_its_connections",
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
      go(&mut window, Some(work.id()), AccountsPanel::Account);
      window.until("what uses Work", |window| {
        joined(window, "account-used-by").contains("Codex · Work")
      });
      assert!(joined(&mut window, "account-used-by-meta").contains("1 in 1 project"));
      more(&mut window);
      click(&mut window, "account-remove");
      window.until("the question in place", |window| {
        shown(window, "account-remove-question")
      });
      assert!(
        shown(&mut window, "account-page"),
        "asked on the account's page, not in a dialog"
      );
      assert!(joined(&mut window, "account-remove-question").contains("Remove Work from Orchester?"));
      click(&mut window, "account-remove-keep");
      window.until("the question gone", |window| !shown(window, "account-remove-question"));
      more(&mut window);
      click(&mut window, "account-remove");
      window.until("the question again", |window| shown(window, "account-remove-question"));
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
      window.until("back on the list", |window| shown(window, "settings-accounts"));
    },
  );
}

#[test]
fn settings_accounts_rename_head_holds_only_the_field_save_and_cancel() {
  in_child(
    "settings_accounts_rename_head_holds_only_the_field_save_and_cancel",
    |root| {
      let mut window = Window::open();
      let work = add_account(
        &window,
        AccountCli::Codex,
        "Work",
        &root.join("accounts").join("codex-work"),
      );
      go(&mut window, Some(work.id()), AccountsPanel::Rename);
      window.until("the name field", |window| shown(window, "account-rename-field"));
      assert!(shown(&mut window, "account-rename-save") && shown(&mut window, "account-rename-cancel"));
      for hidden in ["account-check-again", "account-more"] {
        assert!(
          !shown(&mut window, hidden),
          "{hidden} is not in the head while renaming"
        );
      }
      click(&mut window, "account-rename-cancel");
      window.until("the head again", |window| shown(window, "account-more"));
    },
  );
}

#[test]
fn settings_accounts_says_in_place_why_default_cant_be_removed() {
  in_child("settings_accounts_says_in_place_why_default_cant_be_removed", |_| {
    let mut window = Window::open();
    go(
      &mut window,
      Some(AccountCli::ClaudeCode.default_account()),
      AccountsPanel::Account,
    );
    more(&mut window);
    assert!(joined(&mut window, "account-remove").contains("can't remove it"));
    click(&mut window, "account-remove");
    assert!(
      !shown(&mut window, "account-remove-question"),
      "Default can't be removed"
    );
  });
}

#[test]
fn settings_accounts_offers_choose_folder_for_a_folder_that_is_gone() {
  in_child(
    "settings_accounts_offers_choose_folder_for_a_folder_that_is_gone",
    |root| {
      let mut window = Window::open();
      let folder = root.join("accounts").join("codex-archive");
      let archive = add_account(&window, AccountCli::Codex, "Archive", &folder);
      fs::remove_dir(&folder).expect("the folder goes");
      go(&mut window, Some(archive.id()), AccountsPanel::Account);
      window.until("Folder missing", |window| shown(window, "account-choose-folder"));
      assert!(shown(&mut window, "account-folder-missing-remove"));
      for hidden in ["account-open-folder", "account-sign-in", "account-copy-folder"] {
        assert!(!shown(&mut window, hidden), "{hidden} needs a folder that is there");
      }
      let found = home().join(".codex-archive");
      fs::create_dir_all(&found).expect("where it is now");
      sign_in(&found);
      let workspace = window.launch.workspace().expect("the session").clone();
      let moved = workspace
        .choose_agent_account_folder(archive.id(), &found)
        .expect("chosen");
      assert_eq!(moved.state(), AccountState::SignedIn);
      assert!(matches!(
        moved.folder(),
        orchester_core::account::AccountFolder::Brought(_)
      ));
    },
  );
}

#[test]
fn settings_accounts_sign_in_step_ends_at_signed_in() {
  in_child("settings_accounts_sign_in_step_ends_at_signed_in", |root| {
    let mut window = Window::open();
    let folder = root.join("accounts").join("codex-client");
    let client = add_account(&window, AccountCli::Codex, "Client", &folder);
    sign_in(&folder);
    go(&mut window, Some(client.id()), AccountsPanel::SignIn);
    window.until("the signed-in outcome", |window| shown(window, "account-sign-in-done"));
    for hidden in ["account-sign-in-own", "account-sign-in-later"] {
      assert!(!shown(&mut window, hidden), "{hidden} is gone once signed in");
    }
  });
}

#[test]
fn settings_accounts_sign_in_step_says_the_claude_plan_and_offers_the_folder() {
  in_child(
    "settings_accounts_sign_in_step_says_the_claude_plan_and_offers_the_folder",
    |root| {
      let mut window = Window::open();
      let folder = root.join("accounts").join("claude-freelance");
      let freelance = add_account(&window, AccountCli::ClaudeCode, "Freelance", &folder);
      sign_in(&folder);
      go(&mut window, Some(freelance.id()), AccountsPanel::SignIn);
      window.until("the signed-in outcome", |window| shown(window, "account-sign-in-done"));
      let done = joined(&mut window, "account-sign-in-done");
      assert!(
        done.contains("Max plan") && done.contains("signed in with claude.ai") && done.contains("Runs on Freelance"),
        "{done}"
      );
      assert!(joined(&mut window, "account-sign-in-copy-folder").contains("Copy folder path"));
    },
  );
}

/// *Check now* is a Claude Code account's only (a Codex account is read on its own), says what it costs, and in a
/// window that does not read plan limits on its own, as every test and QA copy is, says that nothing ran.
#[test]
fn settings_accounts_check_now_is_claudes_and_says_when_nothing_ran() {
  in_child(
    "settings_accounts_check_now_is_claudes_and_says_when_nothing_ran",
    |_| {
      let mut window = Window::open();
      go(
        &mut window,
        Some(AccountCli::ClaudeCode.default_account()),
        AccountsPanel::Account,
      );
      window.until("Claude's plan limits", |window| shown(window, "account-plan-check"));
      let note = joined(&mut window, "account-plan-note");
      assert!(
        note.contains("Haiku") && note.contains("never checks on its own"),
        "{note}"
      );
      click(&mut window, "account-plan-check");
      window.until("why nothing ran", |window| {
        joined(window, "account-plan-check-error").contains("doesn't read plan limits on its own")
      });
      go(
        &mut window,
        Some(AccountCli::Codex.default_account()),
        AccountsPanel::Account,
      );
      window.until("Codex's plan limits", |window| {
        joined(window, "account-plan-note").contains("without a turn or tokens")
      });
      assert!(!shown(&mut window, "account-plan-check"), "Codex is read on its own");
      assert!(
        !shown(&mut window, "account-plan-check-error"),
        "the error was Claude's"
      );
    },
  );
}
