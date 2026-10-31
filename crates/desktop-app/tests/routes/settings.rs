//! Settings' six tabs (`App settings · redesign` `iUmo3`, `j3Lre`): *General · Notifications · Accounts · Automation ·
//! Browser · Orchester Cloud*, each a place of its own, with the pages under Accounts and Orchester Cloud; Orchester
//! Cloud keeps the id `orchester` it had when the tab was named Orchester, and Settings without a tab is Automation.

use orchester_app::routes::{AccountsPanel, AccountsView, OrchesterView, Route, Screen};

/// The tabs, and the pages under them that have an address.
pub fn routes() -> Vec<Route> {
  vec![
    Route::Notifications,
    Route::Browser,
    Route::Accounts(AccountsView::default()),
    Route::Accounts(AccountsView {
      account: None,
      panel: AccountsPanel::Add,
    }),
    Route::Orchester(OrchesterView::default()),
    Route::Orchester(OrchesterView::connect()),
  ]
}

#[test]
fn settings_tabs_have_their_documented_paths() {
  for (route, path) in [
    (Route::General, "/settings?tab=general"),
    (Route::Notifications, "/settings?tab=notifications"),
    (Route::Browser, "/settings?tab=browser"),
    (Route::Accounts(AccountsView::default()), "/settings?tab=accounts"),
    (Route::Settings, "/settings"),
    (Route::Orchester(OrchesterView::default()), "/settings?tab=orchester"),
    (
      Route::Orchester(OrchesterView::connect()),
      "/settings?tab=orchester&panel=connect",
    ),
  ] {
    assert_eq!(route.path(), path);
    assert_eq!(Route::parse(path), Ok(route.clone()), "{path}");
    assert_eq!(route.screen(), Screen::Settings, "{path}");
  }
}
