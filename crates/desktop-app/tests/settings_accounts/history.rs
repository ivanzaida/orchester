//! Choosing an account and one of its panels on Settings › Accounts is an entry of the tab's history: Back
//! returns to the account and the panel before, Forward comes back.

use orchester_app::routes::{AccountsPanel, AccountsView, Route};
use orchester_core::{account::AccountCli, id::AgentAccountId};

use super::harness::{Window, click, go, in_child};

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
fn settings_accounts_chosen_accounts_and_panels_are_history() {
  in_child(
    "history::settings_accounts_chosen_accounts_and_panels_are_history",
    |_| {
      let mut window = Window::open();
      let claude = AccountCli::ClaudeCode.default_account();
      let codex = AccountCli::Codex.default_account();
      go(&mut window, None, AccountsPanel::Account);
      click(&mut window, &format!("account-row-{claude}"));
      assert_eq!(now(&window), at(Some(claude), AccountsPanel::Account));
      click(&mut window, &format!("account-row-{codex}"));
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::Account));
      window.until("Codex's details", |window| {
        window.tree.get_element_by_id("account-tab-used-by").is_some()
      });
      click(&mut window, "account-tab-used-by");
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::UsedBy));

      back(&mut window);
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::Account));
      back(&mut window);
      assert_eq!(now(&window), at(Some(claude), AccountsPanel::Account));
      back(&mut window);
      assert_eq!(now(&window), at(None, AccountsPanel::Account));
      forward(&mut window);
      assert_eq!(now(&window), at(Some(claude), AccountsPanel::Account));
      forward(&mut window);
      forward(&mut window);
      assert_eq!(now(&window), at(Some(codex), AccountsPanel::UsedBy));
      assert!(
        window.tree.get_element_by_id("account-tab-used-by").is_some(),
        "Forward shows the panel again"
      );
    },
  );
}
