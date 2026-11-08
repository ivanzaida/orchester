//! `navigate` refuses what names no route, the task page's former tabs and misplaced views, and words each
//! parameter of its refusals.

use super::*;

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

/// The task page's former tabs (ORC-100): Details and Artifacts went into Summary, Memory and Local integration are
/// views (`view`) of Summary and Changes.
#[test]
fn navigate_refuses_the_task_pages_former_tabs_and_misplaced_views() {
  let window = Window::open();
  let navigation = window.navigation();
  let refusals = [
    (json!({ "tab": "details" }), "unknown_tab"),
    (json!({ "tab": "artifacts" }), "unknown_tab"),
    (json!({ "tab": "memory" }), "unknown_tab"),
    (json!({ "tab": "local-integration" }), "unknown_tab"),
    (json!({ "tab": "changes", "view": "memory" }), "invalid_param"),
    (json!({ "view": "artifacts" }), "invalid_param"),
    (json!({ "file": "a.rs", "view": "local-integration" }), "invalid_param"),
  ];
  for (extra, code) in refusals {
    let mut args = json!({ "route": "task", "project": PROJECT, "key": "KON-1" });
    for (name, value) in extra.as_object().expect("an object") {
      args[name] = value.clone();
    }
    let error = navigate(&navigation, &args).expect_err("a refusal");
    assert_eq!(error.code(), code, "{args}");
  }
  assert_eq!(navigation.route_now().path(), "/", "a refusal goes nowhere");
}

/// Modules' places (ORC-124): `view` new opens New module, New task in module keeps its module, and a `tab` or `view`
/// that names nothing where it is given is refused, not left out.
#[test]
fn navigate_opens_modules_places_and_refuses_misplaced_tabs_and_views() {
  let window = Window::open();
  let navigation = window.navigation();
  let module = "7d6c5b4a-3928-4716-8504-f3e2d1c0b9a8";
  let root = format!("/project/{PROJECT}");
  let cases = [
    (
      json!({ "route": "modules", "project": PROJECT, "view": "new" }),
      format!("{root}/modules/new"),
    ),
    (
      json!({ "route": "new_task", "project": PROJECT, "module": module }),
      format!("{root}/tasks/new?module={module}"),
    ),
  ];
  for (args, path) in cases {
    let answer = navigate(&navigation, &args).expect("a valid route");
    assert_eq!(answer["route"]["path"], json!(path), "{args}");
    let mut again = answer["route"]["params"].as_object().expect("params").clone();
    again.insert("route".to_owned(), answer["route"]["name"].clone());
    let repeated = navigate(&navigation, &json!(again)).expect("its params name the same route");
    assert_eq!(
      repeated["route"]["path"],
      json!(path),
      "the params keep the place: {again:?}"
    );
  }
  let refusals = [
    json!({ "view": "edit" }),
    json!({ "view": "burndown" }),
    json!({ "view": "new", "tab": "closed" }),
    json!({ "module": "new", "tab": "summary" }),
    json!({ "module": "new", "view": "edit" }),
    json!({ "module": module, "view": "edit", "tab": "burndown" }),
  ];
  let before = navigation.route_now();
  for extra in refusals {
    let mut args = json!({ "route": "modules", "project": PROJECT });
    for (name, value) in extra.as_object().expect("an object") {
      args[name] = value.clone();
    }
    let error = navigate(&navigation, &args).expect_err("a refusal");
    assert_eq!(error.code(), "invalid_param", "{args}");
  }
  assert_eq!(navigation.route_now(), before, "a refusal goes nowhere");
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
  // The Tasks list's views replaced its workflow tabs (ORC-99): their names are refused, with no alias. Needs you is
  // a view again (ORC-105).
  for old in ["todo", "in_progress", "in_review"] {
    let refused = navigate(
      &navigation,
      &json!({ "route": "tasks", "project": PROJECT, "tab": old }),
    )
    .expect_err("an old tab is refused");
    assert_eq!(refused.code(), "unknown_tab", "{old}");
    assert_eq!(
      refused.to_json()["known"],
      json!(["needs_you", "active", "backlog", "done", "all"]),
      "{old}"
    );
  }
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
