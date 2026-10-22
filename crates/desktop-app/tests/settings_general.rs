//! Settings › General (`hUaXB`): its sections, the theme it keeps in the application database and gives the window,
//! the warning threshold it saves for every project, this computer's version and data, and the header's save state.

mod support;

use orchester_app::{appearance::ThemeSetting, routes::Route};
use support::Window;

/// Settings › General, once its rows have read what they show.
fn general() -> Window {
  let mut window = Window::open();
  window.navigation().go(Route::General);
  window.until("the general tab's rows", |window| {
    window.texts("settings-threshold-value-box").is_some() && window.texts("settings-data-path").is_some()
  });
  window
}

#[test]
fn settings_general_shows_its_sections_with_the_save_state() {
  let mut window = general();
  let texts = window.texts("settings-general").expect("the tab shows");
  for words in [
    "Appearance",
    "Theme",
    "Dark",
    "Light",
    "System",
    "Plan limits",
    "Warning threshold",
    "This computer",
    "Version",
    "Application data",
    "App log",
  ] {
    assert!(texts.iter().any(|text| text == words), "{words} in {texts:?}");
  }
  assert_eq!(window.text("settings-save-state"), "All changes saved");
  assert_eq!(
    window.text("settings-version-value"),
    format!("v{}", env!("CARGO_PKG_VERSION"))
  );
  let database = window.data.database();
  assert!(
    window.text("settings-data-path").contains(&*database.display().to_string()),
    "the session's database: {}",
    window.text("settings-data-path")
  );
  assert!(
    !window.link.input_gate().protecting(),
    "nothing on General is a person's alone"
  );
}

#[test]
fn settings_general_keeps_the_chosen_theme_and_shows_it() {
  let mut window = general();
  let workspace = window.launch.workspace().expect("the session started").clone();
  assert_eq!(workspace.theme().expect("the theme reads"), ThemeSetting::Dark);
  window.press("settings-theme-light");
  window.until("the theme to be kept", |_| {
    workspace.theme().expect("the theme reads") == ThemeSetting::Light
  });
  let appearance = window.link.appearance().expect("the window has its appearance");
  window.until("the window to take it", |_| appearance.setting() == ThemeSetting::Light);
  window.until("the saves to settle", |window| {
    window.text("settings-save-state") == "All changes saved"
  });
}

#[test]
fn settings_general_saves_the_warning_threshold_for_every_project() {
  let mut window = general();
  let workspace = window.launch.workspace().expect("the session started").clone();
  window.until("the stored threshold in the field", |window| {
    window
      .tree
      .get_element_by_id_mut("settings-threshold-value")
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "80")
  });
  window.set("settings-threshold-value", "70");
  window.until("the threshold to be saved", |_| {
    workspace.usage_warning_threshold().expect("the threshold reads").percent() == 70
  });
  window.set("settings-threshold-value", "abc");
  window.until("the field to say why", |window| {
    window.texts("settings-threshold-error").is_some()
  });
  assert_eq!(
    window.text("settings-threshold-error"),
    "Use a whole percentage from 1 to 100."
  );
  assert_eq!(
    workspace.usage_warning_threshold().expect("the threshold reads").percent(),
    70,
    "a value that is not a percentage is not saved"
  );
}

#[test]
fn settings_general_opens_from_its_tab_and_the_save_state_leaves_on_accounts() {
  let mut window = Window::open();
  window.navigation().go(Route::Settings);
  window.until("the automation tab", |window| {
    window.texts("settings-automation").is_some()
  });
  assert_eq!(window.text("settings-save-state"), "All changes saved");
  window.press("settings-tab-general");
  window.until("the general tab", |window| window.texts("settings-general").is_some());
  window.press("settings-tab-accounts");
  window.until("the accounts tab", |window| {
    window.texts("settings-add-account").is_some()
  });
  assert!(
    window.tree.get_element_by_id("settings-save-state").is_none(),
    "Accounts has its action in the save state's place"
  );
}
