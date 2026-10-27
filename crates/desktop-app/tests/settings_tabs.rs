//! Settings' five page tabs (`Tabs` `xEFQ0` in `App settings · redesign` `iUmo3`): *General · Notifications ·
//! Accounts · Automation · Orchester Cloud* in that order, each opening its own place and its column, and the page's
//! title with the save state on the tabs whose changes save as they are made.

mod support;

use orchester_app::routes::{AccountsView, OrchesterView, Route};
use support::Window;

/// Each tab: its element id, its words, the place it opens and the column it shows.
fn tabs() -> [(&'static str, &'static str, Route, &'static str); 5] {
  [
    ("settings-tab-general", "General", Route::General, "settings-general"),
    (
      "settings-tab-notifications",
      "Notifications",
      Route::Notifications,
      "settings-notifications-tab",
    ),
    (
      "settings-tab-accounts",
      "Accounts",
      Route::Accounts(AccountsView::default()),
      "settings-accounts-scroll",
    ),
    (
      "settings-tab-automation",
      "Automation",
      Route::Settings,
      "settings-automation",
    ),
    (
      "settings-tab-orchester",
      "Orchester Cloud",
      Route::Orchester(OrchesterView::default()),
      "orchester-scroll",
    ),
  ]
}

fn left(window: &mut Window, id: &str) -> f32 {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .x
}

#[test]
fn settings_five_tabs_open_their_own_places_in_the_designs_order() {
  let mut window = Window::open();
  window.navigation().go(Route::General);
  window.until("Settings", |window| window.texts("settings-tabs").is_some());
  assert_eq!(window.text("settings-title"), "Settings");
  let order: Vec<f32> = tabs().iter().map(|(id, ..)| left(&mut window, id)).collect();
  assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
  for (id, words, route, column) in tabs() {
    assert!(window.text(id).starts_with(words), "{id}: {}", window.text(id));
    window.press(id);
    assert_eq!(window.navigation().route_now(), route, "{id}");
    window.until(column, |window| window.tree.get_element_by_id(column).is_some());
  }
}

#[test]
fn settings_notifications_is_a_tab_of_its_own_with_the_daily_report() {
  let mut window = Window::open();
  window.navigation().go(Route::Notifications);
  window.until("Notifications", |window| {
    window.texts("settings-notifications-tab").is_some()
  });
  window.until("the daily report", |window| {
    window.texts("settings-daily-report-row").is_some()
  });
  assert_eq!(window.text("settings-save-state"), "All changes saved");
  assert!(
    window.tree.get_element_by_id("settings-daily-report-section").is_some(),
    "the daily report moved here from Automation"
  );
  window.navigation().go(Route::Settings);
  window.until("Automation", |window| window.texts("settings-automation").is_some());
  assert!(
    window.tree.get_element_by_id("settings-daily-report-section").is_none(),
    "Automation no longer holds it"
  );
}
