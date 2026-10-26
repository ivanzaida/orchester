//! An account opens as a page of its own under Settings › Accounts (`03b · Account` `xN1Jy`), and its crumb goes back
//! to the list; each is an entry of the tab's history, so Back and Forward walk them as in a browser.

use orchester_app::routes::{AccountsPanel, AccountsView, Route};
use orchester_core::{account::AccountCli, id::AgentAccountId};

use super::harness::{Window, click, go, in_child, shown};

fn at(account: Option<AgentAccountId>, panel: AccountsPanel) -> Route {
  Route::Accounts(AccountsView { account, panel })
}

fn now(window: &Window) -> Route {
  window.navigation().route_now()
}

fn back(window: &mut Window) {
  window.navigation().back();
  window.pass();
}

fn forward(window: &mut Window) {
  window.navigation().forward();
  window.pass();
}

#[test]
fn settings_accounts_an_account_opens_as_a_page_and_its_crumb_goes_back() {
  in_child(
    "history::settings_accounts_an_account_opens_as_a_page_and_its_crumb_goes_back",
    |_| {
      let mut window = Window::open();
      let claude = AccountCli::ClaudeCode.default_account();
      let codex = AccountCli::Codex.default_account();
      go(&mut window, None, AccountsPanel::Account);
      window.until("the list's rows", |window| {
        shown(window, &format!("account-row-{claude}-open"))
      });
      click(&mut window, &format!("account-row-{claude}-open"));
      assert_eq!(now(&window), at(Some(claude), AccountsPanel::Account));
      window.until("Claude's page", |window| shown(window, "account-page"));
      assert!(!shown(&mut window, "settings-accounts"), "the page takes the list's place");
      click(&mut window, "account-crumb");
      assert_eq!(now(&window), at(None, AccountsPanel::Account), "the crumb goes back to the list");
      window.until("the list", |window| shown(window, "settings-accounts"));
      click(&mut window, &format!("account-row-{codex}-open"));
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::Account));

      back(&mut window);
      assert_eq!(now(&window), at(None, AccountsPanel::Account));
      back(&mut window);
      assert_eq!(now(&window), at(Some(claude), AccountsPanel::Account));
      window.until("Claude's page again", |window| shown(window, "account-page"));
      forward(&mut window);
      forward(&mut window);
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::Account));
      window.until("Codex's page again", |window| shown(window, "account-page"));
      // The tab goes back to the list from a page of it.
      click(&mut window, "settings-tab-accounts");
      assert_eq!(now(&window), at(None, AccountsPanel::Account));
    },
  );
}
