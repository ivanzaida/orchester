//! The app's MCP tools on a window without a screen: `app_state` reports where the window is,
//! `navigate` goes through the tabs' history and refuses with typed errors, `set_theme` recolors the
//! window, and what agents may do follows the settings.

mod support;

use lurq::{app::theme::PaletteColor, mcp::Scope, node::color::Color};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{
    McpSettings, WindowLink,
    settings::WINDOW_CONTROL_VARIABLE,
    tools::{ToolError, app_state, navigate, set_theme},
  },
  settings::AppSettings,
  theme::Mode,
};
use serde_json::{Value, json};
use support::Window;

#[test]
fn window_control_is_on_in_debug_builds_only_unless_the_environment_says_otherwise() {
  assert_eq!(McpSettings::of_build().window_control(), cfg!(debug_assertions));
  let with = |value: &'static str| {
    McpSettings::with_window_control(false)
      .overridden_by(|name| (name == WINDOW_CONTROL_VARIABLE).then(|| value.to_owned()))
  };
  assert!(with("on").window_control());
  assert!(with("1").window_control());
  assert!(!with("maybe").window_control(), "an unreadable value keeps the setting");
  assert!(
    !McpSettings::with_window_control(true)
      .overridden_by(|_| Some("off".to_owned()))
      .window_control()
  );
  assert_eq!(McpSettings::with_window_control(false).scopes(), [Scope::Observe]);
  assert_eq!(
    McpSettings::with_window_control(true).scopes(),
    [Scope::Observe, Scope::Interact]
  );
}

#[test]
fn app_state_reports_route_tabs_theme_and_window_size() {
  let mut window = Window::open();
  let appearance = Appearance::new(ThemeSetting::Dark, None, None);
  let state = app_state(&window.navigation(), &appearance, &mut window.tree);
  assert_eq!(
    state,
    json!({
      "route": { "name": "all_projects", "path": "/", "params": {} },
      "history": { "can_back": false, "can_forward": false },
      "tabs": [{ "id": 1, "active": true, "route": { "name": "all_projects", "path": "/", "params": {} } }],
      "theme": { "setting": "dark", "mode": "dark" },
      "window": { "width": 1440.0, "height": 900.0, "scale_factor": 1.0 },
    })
  );
}

#[test]
fn navigate_takes_the_active_tab_to_a_named_route() {
  let mut window = Window::open();
  let navigation = window.navigation();
  let answer = navigate(
    &navigation,
    &json!({ "route": "task", "project": "kontur", "key": "KON-12", "tab": "summary" }),
  )
  .expect("a valid route");
  let route = json!({
    "name": "task",
    "path": "/project/kontur/tasks/KON-12/summary",
    "params": { "project": "kontur", "key": "KON-12", "tab": "summary" },
  });
  assert_eq!(answer, json!({ "route": route }));
  window.pass();
  assert_eq!(
    window.texts("screen-placeholder").map(|texts| texts[0].clone()),
    Some("KON-12 · Summary".to_owned())
  );
  let state = app_state(
    &navigation,
    &Appearance::new(ThemeSetting::Dark, None, None),
    &mut window.tree,
  );
  assert_eq!(state["route"], route);
  assert_eq!(state["history"], json!({ "can_back": true, "can_forward": false }));
  navigate(&navigation, &json!({ "route": "overview", "project": "atlas" })).expect("a valid route");
  assert_eq!(navigation.route_now().path(), "/project/atlas");
}

#[test]
fn navigate_refuses_what_names_no_route_with_a_stable_code() {
  let window = Window::open();
  let navigation = window.navigation();
  let refusals = [
    (json!({ "route": "dashboard" }), "unknown_route"),
    (json!({}), "missing_param"),
    (json!({ "route": "tasks" }), "missing_param"),
    (json!({ "route": "task", "project": "kontur" }), "missing_param"),
    (json!({ "route": "settings", "project": "kontur" }), "unexpected_param"),
    (json!({ "route": "overview", "project": "a/b" }), "invalid_param"),
    (
      json!({ "route": "task", "project": "kontur", "key": "KON-1", "tab": "files" }),
      "unknown_tab",
    ),
    (json!({ "route": 7 }), "not_a_string"),
    (json!(["overview"]), "not_an_object"),
  ];
  for (args, code) in refusals {
    let error = navigate(&navigation, &args).expect_err("a refusal");
    assert_eq!(error.code(), code, "{args}");
    assert_eq!(error.to_json()["code"], code);
  }
  let unknown = navigate(&navigation, &json!({ "route": "dashboard" })).expect_err("a refusal");
  let known = &unknown.to_json()["known"];
  assert!(
    known
      .as_array()
      .is_some_and(|names| names.contains(&json!("pull_requests")))
  );
  assert_eq!(navigation.route_now().path(), "/", "a refusal goes nowhere");
}

#[test]
fn set_theme_switches_the_colors_and_keeps_the_choice() {
  let window = Window::open();
  let dir = tempfile::tempdir().expect("a temporary folder");
  let settings = AppSettings::open(&dir.path().join("orchester.db")).expect("the settings open");
  let appearance = Appearance::new(ThemeSetting::Dark, Some(Mode::Dark), Some(settings.clone()));
  let theme = window.app.theme();
  let answer = set_theme(&appearance, theme, &json!({ "theme": "light" })).expect("a known theme");
  assert_eq!(answer, json!({ "setting": "light", "mode": "light" }));
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#E4E4E8")
  );
  assert_eq!(settings.theme().expect("stored"), ThemeSetting::Light);
  let error = set_theme(&appearance, theme, &json!({ "theme": "sepia" })).expect_err("an unknown theme");
  assert_eq!(error.code(), "unknown_theme");
  assert_eq!(error.to_json()["known"], json!(["dark", "light", "system"]));
  assert_eq!(appearance.setting(), ThemeSetting::Light, "a refusal keeps the theme");
  let unsaved = Appearance::new(ThemeSetting::Dark, None, None);
  let error = set_theme(&unsaved, theme, &json!({ "theme": "dark" })).expect_err("nowhere to keep it");
  assert_eq!(error.code(), "not_saved");
  assert_eq!(
    theme.palette_color(PaletteColor::SurfaceBase),
    Color::from_hex("#E4E4E8"),
    "nothing changed"
  );
}

#[test]
fn window_control_follows_the_stored_setting_then_the_environment() {
  let stored_off = McpSettings::with_window_control(true).with_stored(Some(false));
  assert!(!stored_off.window_control());
  assert_eq!(
    McpSettings::of_build().with_stored(None),
    McpSettings::of_build(),
    "unset: the build decides"
  );
  let overridden = stored_off.overridden_by(|name| (name == WINDOW_CONTROL_VARIABLE).then(|| "on".to_owned()));
  assert!(
    overridden.window_control(),
    "the environment overrides the stored value for one run"
  );
}

#[test]
fn tools_wait_for_the_window() {
  let error = WindowLink::default()
    .navigation()
    .map(|_| Value::Null)
    .expect_err("no window yet");
  assert_eq!(error, ToolError::NotReady);
  assert_eq!(error.to_json()["code"], "not_ready");
}
