//! API providers in Settings › Accounts (`Agents on accounts` A1 `aOR4w`, B2 `OfUhN`): *Add API provider* opens
//! its own page; a name, a base URL, a key and at least one model add the account, which then stands under its CLI's
//! sign-ins in the list, with its host and how many models it serves, and nothing of its key.
//! `cargo test -p orchester-app --test settings_accounts settings_accounts_api`.

use orchester_app::routes::{AccountsPanel, AccountsView, Route};
use orchester_core::{account::ApiSetup, id::ApiAccountId};
use orchester_store::AppDb;

use super::harness::{Window, click, go, in_child, joined, set, shown};

/// The key the test types, which nothing the window shows may hold.
const KEY: &str = "sk-typed-by-the-settings-test";

#[test]
fn settings_accounts_api_a_provider_is_added_with_its_key_and_listed_under_its_cli() {
  in_child(
    "api::settings_accounts_api_a_provider_is_added_with_its_key_and_listed_under_its_cli",
    |_root| {
      let mut window = Window::open();
      // Tall enough for *Add provider* under the page's fields to take a press.
      window.tree.resize(1440, 1600);
      go(&mut window, None, AccountsPanel::Account);
      assert!(
        shown(&mut window, "settings-add-account"),
        "a sign-in is still added here"
      );
      // The list stands once the accounts are read; until then only its skeleton does. *Add account* is a menu of
      // two there (A2 `It53u`): a sign-in, or an API provider.
      window.until("the list", |window| shown(window, "accounts-group-claude_code"));
      click(&mut window, "settings-add-account");
      window.until("the menu", |window| shown(window, "settings-add-api"));
      assert!(shown(&mut window, "settings-add-sign-in"));
      click(&mut window, "settings-add-api");
      window.until("the page", |window| shown(window, "api-add-page"));
      assert_eq!(
        window.navigation().route_now(),
        Route::Accounts(AccountsView {
          account: None,
          provider: None,
          panel: AccountsPanel::AddApi,
        })
      );

      // Nothing is added before every field can be saved, and the page says what it still needs.
      assert_eq!(joined(&mut window, "api-add-needs"), "Name it first");
      set(&mut window, "api-add-name", "DeepSeek");
      set(
        &mut window,
        "connection-provider-url",
        "https://api.deepseek.com/anthropic",
      );
      set(&mut window, "connection-provider-secret-value", KEY);
      click(&mut window, "api-add-save");
      window.pass();
      let accounts = |window: &Window| {
        AppDb::open(&window.data.database())
          .expect("the application database")
          .api_accounts()
          .expect("read")
      };
      assert!(accounts(&window).is_empty(), "no model yet: nothing is added");
      assert_eq!(joined(&mut window, "api-add-needs"), "Name at least one model");

      set(&mut window, "connection-provider-model", "deepseek-chat");
      click(&mut window, "connection-provider-add-model");
      window.until("the model's pill", |window| {
        shown(window, "connection-provider-model-0")
      });
      click(&mut window, "api-add-save");
      window.until("the list again", |window| shown(window, "settings-accounts"));

      let added = accounts(&window);
      assert_eq!(added.len(), 1);
      let account = &added[0];
      assert_eq!(account.name().as_str(), "DeepSeek");
      assert!(
        matches!(account.setup(), ApiSetup::Cli { models, .. } if models.len() == 1),
        "{:?}",
        account.setup()
      );
      let row = format!("api-row-{}-row", account.id());
      window.until("its row", |window| shown(window, &row));
      let said = joined(&mut window, &row);
      assert!(
        said.contains("DeepSeek") && said.contains("API provider · api.deepseek.com · 1 model"),
        "{said}"
      );
      assert!(said.contains("Key set"), "{said}");
      assert!(!said.contains(KEY), "the key is shown nowhere");
      assert!(
        joined(&mut window, "accounts-group-claude_code").contains("1 account · 1 API provider"),
        "{}",
        joined(&mut window, "accounts-group-claude_code")
      );

      provider_page(&mut window, account.id(), &row);

      // The page starts over: what was typed, the key first, is gone from it.
      go(&mut window, None, AccountsPanel::AddApi);
      window.until("the page", |window| shown(window, "api-add-page"));
      assert!(!joined(&mut window, "api-add-page").contains(KEY));
      assert!(!shown(&mut window, "connection-provider-model-0"), "no model is left");
    },
  );
}

/// The provider's row opens its page (B1 `gZTuX`): what reaches the API, how its key stands and nothing of the
/// key; *Rename* is in the page's menu and takes at once.
fn provider_page(window: &mut Window, id: ApiAccountId, row: &str) {
  click(window, row);
  window.until("its page", |window| shown(window, "provider-page"));
  assert_eq!(
    window.navigation().route_now(),
    Route::Accounts(AccountsView::of_provider(id, AccountsPanel::Account))
  );
  window.until("what runs on it", |window| shown(window, "provider-used-by"));
  let page = joined(window, "provider-page");
  for words in [
    "DeepSeek",
    "Key set",
    "used with Claude Code",
    "Claude Code · Anthropic Messages API",
    "https://api.deepseek.com/anthropic",
    "deepseek-chat",
    "api.deepseek.com · agreed per project at its first run",
    "No project runs a profile on this account yet",
  ] {
    assert!(page.contains(words), "{words} in {page}");
  }
  assert!(!page.contains(KEY), "the key is shown nowhere");
  // Rename is in the page's menu and takes at once.
  click(window, "provider-more");
  window.until("the menu", |window| shown(window, "provider-rename"));
  assert!(shown(window, "provider-edit") && shown(window, "provider-remove"));
  click(window, "provider-rename");
  window.until("the name's field", |window| shown(window, "provider-rename-field"));
  set(window, "provider-rename-field", "DeepSeek EU");
  click(window, "provider-rename-save");
  window.until("the new name", |window| {
    AppDb::open(&window.data.database())
      .expect("the application database")
      .api_accounts()
      .expect("read")
      .first()
      .is_some_and(|account| account.name().as_str() == "DeepSeek EU")
  });
  window.until("the page again", |window| shown(window, "provider-head-title"));
}

/// *Used with* offers Orchester's own agent too: its account takes a name and a key (the base URL starts at
/// Anthropic's, and no model is needed), is kept as a native setup, and stands in a group of its own, *Model APIs*,
/// with nothing of its key.
#[test]
fn settings_accounts_api_an_account_of_orchesters_own_agent_stands_under_model_apis() {
  in_child(
    "api::settings_accounts_api_an_account_of_orchesters_own_agent_stands_under_model_apis",
    |_root| {
      let mut window = Window::open();
      go(&mut window, None, AccountsPanel::AddApi);
      window.until("the page", |window| shown(window, "api-add-page"));
      assert!(
        !shown(&mut window, "accounts-group-model-apis"),
        "no group while there is no such account"
      );
      // The own agent's fields are more than the window's 900 shows: a taller one keeps *Add provider* under the
      // pointer, where a person scrolls.
      window.tree.resize(1440, 1600);
      window.pass();
      click(&mut window, "api-add-cli-own");
      window.until("the own agent's fields", |window| shown(window, "connection-presets"));
      assert_eq!(joined(&mut window, "api-add-needs"), "Name it first");
      set(&mut window, "api-add-name", "Anthropic");
      window.until("the key is asked for", |window| {
        joined(window, "api-add-needs") == "Type its API key"
      });
      set(&mut window, "connection-provider-secret-value", KEY);
      click(&mut window, "api-add-save");
      window.until("the list again", |window| shown(window, "settings-accounts"));

      let added = AppDb::open(&window.data.database())
        .expect("the application database")
        .api_accounts()
        .expect("read");
      assert_eq!(added.len(), 1);
      let account = &added[0];
      assert!(
        matches!(account.setup(), ApiSetup::Native { base_url, .. } if base_url.host() == "api.anthropic.com"),
        "{:?}",
        account.setup()
      );
      window.until("its group", |window| shown(window, "accounts-group-model-apis"));
      let said = joined(&mut window, "accounts-group-model-apis");
      assert!(
        said.contains("Model APIs") && said.contains("Anthropic API · api.anthropic.com · no models listed yet"),
        "{said}"
      );
      assert!(said.contains("Key set") && !said.contains(KEY), "{said}");
    },
  );
}
