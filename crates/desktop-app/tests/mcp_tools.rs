//! The app's MCP tools on a window without a screen: `app_state` reports where the window is and the
//! projects its tabs show, `navigate` goes through the tabs' history and refuses with typed errors,
//! `set_theme` recolors the window, `hold_reads` holds the project feeds' reads, and what agents may do
//! follows the settings.

mod support;

use std::sync::{Arc, OnceLock};

use lurq::{app::theme::PaletteColor, mcp::Scope, node::color::Color};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  mcp::{
    McpSettings, ThisRun, WindowLink,
    domain::DATA_SCOPE,
    settings::WINDOW_CONTROL_VARIABLE,
    tools::{ToolError, app_state, hold_reads, navigate, set_theme},
  },
  project::hold::ReadHold,
  theme::Mode,
};
use orchester_store::AppDb;
use orchester_workspace::workspace::Workspace;
use serde_json::{Value, json};
use support::Window;

const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

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
  let data = Scope::custom(DATA_SCOPE);
  assert_eq!(
    McpSettings::with_window_control(false).scopes(),
    [Scope::Observe, data.clone()],
    "changing project data is not controlling the window"
  );
  assert_eq!(
    McpSettings::with_window_control(true).scopes(),
    [Scope::Observe, data, Scope::Interact]
  );
}

#[test]
fn app_state_reports_route_tabs_theme_and_window_size() {
  let mut window = Window::open();
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
  let settings = McpSettings::with_window_control(false).with_stored(Some(true));
  let state = app_state(&window.link, &appearance, settings, &mut window.tree).expect("a session's window");
  assert_eq!(
    state,
    json!({
      "route": { "name": "all_projects", "path": "/", "params": {} },
      "history": { "can_back": false, "can_forward": false },
      "tabs": [{ "id": 1, "active": true, "route": { "name": "all_projects", "path": "/", "params": {} } }],
      "theme": { "setting": "dark", "mode": "dark" },
      "mcp": { "window_control": true, "source": "stored" },
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
    &json!({ "route": "task", "project": PROJECT, "key": "KON-12", "tab": "summary" }),
  )
  .expect("a valid route");
  let route = json!({
    "name": "task",
    "path": format!("/project/{PROJECT}/tasks/KON-12/summary"),
    "params": { "project": PROJECT, "key": "KON-12", "tab": "summary" },
  });
  assert_eq!(answer, json!({ "route": route }));
  // The project has no tasks: once its tasks are read, the task's page says KON-12 is not there.
  window.until("the task to be looked up", |window| {
    window.texts("task-not-found-title").is_some()
  });
  assert_eq!(
    window.texts("task-not-found-title"),
    Some(vec!["No task KON-12 in Kontur".to_owned()])
  );
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
  let state = |window: &mut Window| {
    app_state(&window.link, &appearance, McpSettings::of_build(), &mut window.tree).expect("a session's window")
  };
  window.until("the runner to fail", |window| {
    state(window)["tabs"][0]["project"]["runner"]["state"] == "failed"
  });
  let state = state(&mut window);
  assert_eq!(state["route"], route);
  assert_eq!(state["history"], json!({ "can_back": true, "can_forward": false }));
  let repo = window.data.repo("repo-kontur").display().to_string();
  assert_eq!(
    state["tabs"][0]["project"],
    json!({
      "name": "Kontur",
      "path": repo,
      "state": "open",
      "runner": { "state": "failed", "cause": "missing_executable" },
    }),
    "the test's runner program does not exist"
  );
  navigate(
    &navigation,
    &json!({ "route": "overview", "project": "0f1e2d3c-4b5a-4968-8776-655443322110" }),
  )
  .expect("a valid route");
  assert_eq!(
    navigation.route_now().path(),
    "/project/0f1e2d3c-4b5a-4968-8776-655443322110"
  );
}

#[test]
fn navigate_takes_the_parameters_each_screen_has_and_answers_them_back() {
  let mut window = Window::open();
  let navigation = window.navigation();
  let root = format!("/project/{PROJECT}");
  let cases = [
    (
      json!({ "route": "overview", "project": PROJECT, "tab": "needs_you" }),
      format!("{root}?tab=needs_you"),
      json!({ "project": PROJECT, "tab": "needs_you" }),
    ),
    (
      json!({ "route": "tasks", "project": PROJECT, "tab": "in_review", "q": "retry & jitter", "highlight": "KON-13" }),
      format!("{root}/tasks?tab=in_review&q=retry%20%26%20jitter&highlight=KON-13"),
      json!({ "project": PROJECT, "tab": "in_review", "q": "retry & jitter", "highlight": "KON-13" }),
    ),
    (
      json!({ "route": "new_task", "project": PROJECT }),
      format!("{root}/tasks/new"),
      json!({ "project": PROJECT }),
    ),
    (
      json!({ "route": "edit_task", "project": PROJECT, "key": "KON-7" }),
      format!("{root}/tasks/KON-7/edit"),
      json!({ "project": PROJECT, "key": "KON-7" }),
    ),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-12", "file": "src/a b.rs", "from": "worktree" }),
      format!("{root}/tasks/KON-12/changes?file=src/a%20b.rs&from=worktree"),
      json!({ "project": PROJECT, "key": "KON-12", "tab": "changes", "file": "src/a b.rs", "from": "worktree" }),
    ),
    (
      json!({ "route": "files", "project": PROJECT, "file": "src/a b.rs", "at": "base", "line": "3-5" }),
      format!("{root}/files/src/a%20b.rs?at=base&line=3-5"),
      json!({ "project": PROJECT, "file": "src/a b.rs", "at": "base", "line": "3-5" }),
    ),
    (
      json!({ "route": "usage", "project": PROJECT, "query": "group=task&model=gpt%205&period=30d" }),
      format!("{root}/usage?period=30d&group=task&model=gpt%205"),
      json!({ "project": PROJECT, "query": "period=30d&group=task&model=gpt%205" }),
    ),
    (
      json!({ "route": "memory", "project": PROJECT, "tab": "waiting" }),
      format!("{root}/memory?tab=waiting"),
      json!({ "project": PROJECT, "tab": "waiting" }),
    ),
    (
      json!({ "route": "settings", "tab": "general" }),
      "/settings?tab=general".to_owned(),
      json!({ "tab": "general" }),
    ),
  ];
  for (args, path, params) in cases {
    let answer = navigate(&navigation, &args).expect("a valid route");
    assert_eq!(answer["route"]["path"], json!(path), "{args}");
    assert_eq!(answer["route"]["params"], params, "{args}");
    let mut again = params.as_object().expect("params are an object").clone();
    again.insert("route".to_owned(), args["route"].clone());
    let repeated = navigate(&navigation, &json!(again)).expect("its params name the same route");
    assert_eq!(repeated["route"]["path"], json!(path), "{args}");
    let screen = format!("screen-{}", args["route"].as_str().expect("a route name"));
    window.until(&screen, |window| window.texts(&screen).is_some());
  }
  assert!(
    window.texts("screen-edit_task").is_none(),
    "only the active route's screen is shown"
  );
}

#[test]
fn navigate_opens_every_projects_usage_without_a_project() {
  let window = Window::open();
  let navigation = window.navigation();
  let answer = navigate(&navigation, &json!({ "route": "usage", "query": "group=agent" })).expect("a valid route");
  assert_eq!(answer["route"]["path"], json!("/usage?group=agent"));
  assert_eq!(answer["route"]["name"], json!("usage"));
  assert_eq!(answer["route"]["params"], json!({ "query": "group=agent" }));
}

#[test]
fn navigate_refuses_what_names_no_route_with_a_stable_code() {
  let window = Window::open();
  let navigation = window.navigation();
  let refusals = [
    (json!({ "route": "dashboard" }), "unknown_route"),
    (json!({}), "missing_param"),
    (json!({ "route": "tasks" }), "missing_param"),
    (json!({ "route": "task", "project": PROJECT }), "missing_param"),
    (json!({ "route": "settings", "project": PROJECT }), "unexpected_param"),
    (json!({ "route": "overview", "project": "a/b" }), "invalid_param"),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-1", "tab": "files" }),
      "unknown_tab",
    ),
    (
      json!({ "route": "overview", "project": PROJECT, "tab": "all" }),
      "unknown_tab",
    ),
    (
      json!({ "route": "tasks", "project": PROJECT, "tab": "running" }),
      "unknown_tab",
    ),
    (
      json!({ "route": "tasks", "project": PROJECT, "highlight": "kon-1" }),
      "invalid_param",
    ),
    (
      json!({ "route": "memory", "project": PROJECT, "tab": "queue" }),
      "unknown_tab",
    ),
    (json!({ "route": "tasks", "project": PROJECT, "q": 7 }), "not_a_string"),
    (
      json!({ "route": "new_task", "project": PROJECT, "key": "KON-1" }),
      "unexpected_param",
    ),
    (json!({ "route": "edit_task", "project": PROJECT }), "missing_param"),
    (
      json!({ "route": "edit_task", "project": PROJECT, "key": "KON" }),
      "invalid_param",
    ),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-1", "tab": "summary", "file": "a.rs" }),
      "invalid_param",
    ),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-1", "from": "worktree" }),
      "invalid_param",
    ),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-1", "file": "a.rs", "from": "disk" }),
      "invalid_param",
    ),
    (
      json!({ "route": "task", "project": PROJECT, "key": "KON-1", "file": "" }),
      "invalid_param",
    ),
    (json!({ "route": 7 }), "not_a_string"),
    (json!(["overview"]), "not_an_object"),
  ];
  for (args, code) in refusals {
    let error = navigate(&navigation, &args).expect_err("a refusal");
    assert_eq!(error.code(), code, "{args}");
    assert_eq!(error.to_json()["code"], code);
  }
  assert_eq!(navigation.route_now().path(), "/", "a refusal goes nowhere");
}

#[test]
fn navigate_refusals_list_the_known_names_and_word_each_parameter() {
  let window = Window::open();
  let navigation = window.navigation();
  let unknown = navigate(&navigation, &json!({ "route": "dashboard" })).expect_err("a refusal");
  let known = &unknown.to_json()["known"];
  assert!(
    known
      .as_array()
      .is_some_and(|names| names.contains(&json!("pull_requests")))
  );
  let tab = navigate(
    &navigation,
    &json!({ "route": "overview", "project": PROJECT, "tab": "all" }),
  )
  .expect_err("a refusal");
  assert_eq!(
    tab.to_json()["known"],
    json!(["running", "needs_you", "queued", "failed"]),
    "the screen's own tabs"
  );
  let message = |args: Value| navigate(&navigation, &args).expect_err("a refusal").to_string();
  assert!(
    message(json!({ "route": "overview", "project": "a/b" })).starts_with("`project`: `a/b` is not a project id"),
    "invalid values are worded per parameter"
  );
  assert_eq!(
    message(json!({ "route": "task", "project": PROJECT, "key": "KON-1", "file": "" })),
    "`file`: the path is empty"
  );
}

#[test]
fn set_theme_switches_the_colors_and_keeps_the_choice() {
  let window = Window::open();
  let dir = tempfile::tempdir().expect("a temporary folder");
  let settings = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("the settings open"));
  let appearance = Appearance::new(
    ThemeSetting::Dark,
    Arc::new(OnceLock::from(Mode::Dark)),
    Some(settings.clone()),
  );
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
  let unsaved = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
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
fn window_control_settings_show_the_next_start_and_what_this_run_has() {
  assert_eq!(
    McpSettings::next_start(None),
    cfg!(debug_assertions),
    "the build's default until set"
  );
  assert!(!McpSettings::next_start(Some(false)));
  let started_on = McpSettings::with_window_control(true);
  assert_eq!(started_on.this_run(true), ThisRun::AsStarted);
  assert_eq!(
    started_on.this_run(false),
    ThisRun::NextStart(false),
    "a change applies from the next start"
  );
  let by_variable = started_on.overridden_by(|name| (name == WINDOW_CONTROL_VARIABLE).then(|| "off".to_owned()));
  assert_eq!(
    by_variable.this_run(true),
    ThisRun::Environment(false),
    "the variable decides this run whatever the setting says"
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
  let startup = WindowLink::default();
  startup.attach_startup("already_open");
  let error = startup.navigation().map(|_| Value::Null).expect_err("no tabs");
  assert_eq!(error.to_json()["code"], "no_session");
}

#[test]
fn hold_reads_holds_and_releases_the_feeds_reads() {
  let hold = ReadHold::default();
  assert_eq!(
    hold_reads(&hold, &json!({ "held": true })).expect("a flag"),
    json!({ "held": true })
  );
  assert!(hold.is_held());
  assert_eq!(
    hold_reads(&hold, &json!({ "held": false })).expect("a flag"),
    json!({ "held": false })
  );
  assert!(!hold.is_held());
  let error = hold_reads(&hold, &json!({ "held": "yes" })).expect_err("not a flag");
  assert_eq!(error.to_json()["code"], "not_a_bool");
  let error = hold_reads(&hold, &json!({})).expect_err("no flag");
  assert_eq!(error.to_json()["code"], "missing_param");
}
#[test]
fn navigate_skills_forms_round_trip_their_parameters() {
  let window = Window::open();
  let navigation = window.navigation();
  for skill in ["new", "import", PROJECT] {
    let args = json!({ "route": "skills", "project": PROJECT, "skill": skill });
    let answer = navigate(&navigation, &args).expect("Skills route");
    assert_eq!(
      answer["route"]["path"],
      format!("/project/{PROJECT}/skills?skill={skill}")
    );
    assert_eq!(answer["route"]["params"], json!({ "project": PROJECT, "skill": skill }));
    assert_eq!(
      orchester_app::routes::Route::parse(&navigation.route_now().path()),
      Ok(navigation.route_now())
    );
  }
}
