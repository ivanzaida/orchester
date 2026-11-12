//! A team admin makes and edits an HTTP request and a database query tool (`Agents · v2` A32 `gLLEv`, A33 `t2RMXK`,
//! A34 `WUuEj`, A35 `YBG4T`), against the stand-in team server: Create publishes v1 with the secret typed by name and
//! its access, Save the next version. On the QA state `team-tools`.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_tools`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::Window,
};
use orchester_core::{resource::ResourceKind, store::ResourceStore};
use serde_json::{Value, json};

use super::{
  hub_edit::{db, opened},
  team::State,
};

const STATE: (&str, State) = ("team-tools", State::TeamEdit);
const CREATE: &str = "team-hub-new-create";
const SAVE: &str = "team-hub-edit-save";

/// The window on the hub's new tool form.
fn new_tool() -> (Window, TeamServer) {
  let (mut window, server, (project, _)) = opened(STATE, "team-hub-panel-title", |_| {
    (HubView::new(HubFilter::Tools, None), None)
  });
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::adding(HubFilter::Tools)),
  ));
  window.until("the form", |window| window.texts(CREATE).is_some());
  tall(&mut window);
  (window, server)
}

/// Tall enough that the form's *Access* is in the window, where a pointer presses it.
fn tall(window: &mut Window) {
  window.tree.resize(1440, 2400);
  window.pass();
}

/// The window on the panel of the team tool `name`, once its field `shown` is there.
fn edited(name: &'static str, shown: &str) -> (Window, TeamServer) {
  let (mut window, server, _) = opened(STATE, shown, move |window| {
    let id = db(window)
      .resources(ResourceKind::Tool)
      .expect("resources")
      .into_iter()
      .find(|resource| resource.name().as_str() == name)
      .expect("the seed's tool")
      .id();
    (HubView::new(HubFilter::Tools, Some(id)), Some(id))
  });
  tall(&mut window);
  (window, server)
}

/// The versions the stand-in holds of the resource named `name`, once it holds `count` of them.
fn versions(window: &mut Window, server: &TeamServer, (name, count): (&str, usize)) -> Vec<Value> {
  let held = |server: &TeamServer| {
    server
      .resources("kontur")
      .into_iter()
      .find(|(resource, _)| resource["name"] == name)
      .map(|(_, versions)| versions)
      .unwrap_or_default()
  };
  window.until("the stand-in to hold the versions", |_| held(server).len() == count);
  held(server)
}

#[test]
fn team_qa_hub_tools_an_http_request_is_published_with_its_secret_by_name() {
  let (mut window, server) = new_tool();
  window.set("team-new-tool-name-input", "github-issue");
  window.press("team-new-tool-kind-value-http");
  window.until("the request's fields", |window| {
    window.texts("team-new-tool-url-input").is_some()
  });
  window.press("team-new-tool-method-value-post");
  window.set(
    "team-new-tool-url-input",
    "https://api.github.com/repos/acme/kontur/issues",
  );
  window.set("team-new-tool-host-input", "api.github.com");
  window.press("team-new-tool-auth-value-bearer");
  window.until("the token's field", |window| {
    window.texts("team-new-tool-secret-input").is_some()
  });
  window.set("team-new-tool-secret-input", "GITHUB_TOKEN");
  window.set("team-new-tool-inputs-input", "title → body title");
  window.set("team-new-tool-outputs-input", "link ← body html_url");
  window.press("team-new-tool-environment-value-production");
  window.until("the computer note to name the secret", |window| {
    window.text("team-new-computer-text") == "Each computer then adds the value of GITHUB_TOKEN in Production."
  });
  window.press(CREATE);
  let body = &versions(&mut window, &server, ("github-issue", 1))[0]["body"];
  let http = &body["action"]["http"];
  assert_eq!(http["method"], json!("POST"));
  assert_eq!(http["auth"], json!({"bearer": {"secret": "GITHUB_TOKEN"}}));
  assert_eq!(http["inputs"], json!({"title": {"body": "title"}}));
  assert_eq!(http["outputs"], json!({"link": {"body": "html_url"}}));
  assert_eq!(body["access"]["environment"], json!("Production"));
  assert_eq!(body["access"]["approval"], json!("once_per_run"));
}

#[test]
fn team_qa_hub_tools_a_database_query_is_published_with_its_credentials_by_name() {
  let (mut window, server) = new_tool();
  window.set("team-new-tool-name-input", "orders-today");
  window.press("team-new-tool-kind-value-database");
  window.until("the query's fields", |window| {
    window.texts("team-new-tool-query-input").is_some()
  });
  window.set("team-new-tool-secret-input", "ORDERS_DB");
  window.set("team-new-tool-query-input", "select id from orders");
  window.press("team-new-tool-window-value-minutes");
  window.until("the minutes' field", |window| {
    window.texts("team-new-tool-minutes-input").is_some()
  });
  window.set("team-new-tool-minutes-input", "15");
  window.press(CREATE);
  let body = &versions(&mut window, &server, ("orders-today", 1))[0]["body"];
  assert_eq!(
    body["action"]["database_query"],
    json!({"credentials": "ORDERS_DB", "query": "select id from orders"})
  );
  assert_eq!(body["access"]["window"], json!({"minutes": 15}));
}

#[test]
fn team_qa_hub_tools_an_admin_saves_an_http_request_and_a_query_as_their_next_versions() {
  let (mut window, server) = edited("sentry-issues", "team-hub-from-team-url-input");
  window.set(
    "team-hub-from-team-url-input",
    "https://sentry.example.test/api/0/projects/acme/kontur/events/",
  );
  window.until("Save to say what it publishes", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("v2"))
  });
  window.press(SAVE);
  let body = &versions(&mut window, &server, ("sentry-issues", 2))[1]["body"];
  assert_eq!(
    body["action"]["http"]["url"],
    json!("https://sentry.example.test/api/0/projects/acme/kontur/events/")
  );
  assert_eq!(
    body["action"]["http"]["auth"],
    json!({"bearer": {"secret": "SENTRY_TOKEN"}}),
    "the secret keeps its name"
  );

  let (mut window, server) = edited("orders-report", "team-hub-from-team-query-input");
  assert_eq!(window.text("team-hub-from-team-secret-input"), "ORDERS_DB");
  window.set("team-hub-from-team-query-input", "select status from orders");
  window.press("team-hub-from-team-environment-value-production");
  window.until("Save to say what it publishes", |window| {
    window
      .texts("team-hub-actions-hint")
      .is_some_and(|said| said.join(" ").contains("v2"))
  });
  window.press(SAVE);
  let body = &versions(&mut window, &server, ("orders-report", 2))[1]["body"];
  assert_eq!(
    body["action"]["database_query"]["query"],
    json!("select status from orders")
  );
  assert_eq!(body["access"]["environment"], json!("Production"));
}
