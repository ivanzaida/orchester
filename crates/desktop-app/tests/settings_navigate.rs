//! `navigate` to Settings' sub-pages and questions in place (ORC-104), so a QA copy reaches each without a click: an
//! account's page by its id, name or a CLI's Default, *Add account*, a team server's page, *Connect a server* at a
//! step, an invite's landing, and *Rotate the token?*, *Sign out…?* and *Decline the invite?*; each route's params
//! name it back, and what a tab does not take is refused.
//!
//! `cargo test -p orchester-app --test settings_navigate settings_navigate`

mod support;

use orchester_app::{
  mcp::tools::{Lookup, navigate, navigate_with},
  routes::{ConnectStep, OrchesterAsk, OrchesterPanel, OrchesterView, Route},
};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};
use orchester_store::AppDb;
use serde_json::{Value, json};
use support::Window;

const SERVER: &str = "0191c7c0-0000-7000-8000-0000000005e1";
const INVITE: &str = "0191c7c0-0000-7000-8000-00000000017e";

#[test]
fn settings_navigate_opens_sub_pages_and_questions_and_names_them_back() {
  let window = Window::open();
  let navigation = window.navigation();
  let claude = AccountCli::ClaudeCode.default_account().to_string();
  let cases = [
    (
      json!({ "route": "settings", "tab": "accounts", "account": "claude-default", "panel": "rename" }),
      format!("/settings?tab=accounts&account={claude}&panel=rename"),
      json!({ "tab": "accounts", "account": claude, "panel": "rename" }),
    ),
    (
      json!({ "route": "settings", "tab": "accounts", "panel": "add" }),
      "/settings?tab=accounts&panel=add".to_owned(),
      json!({ "tab": "accounts", "panel": "add" }),
    ),
    (
      json!({ "route": "settings", "tab": "automation", "ask": "rotate-token" }),
      "/settings?ask=rotate-token".to_owned(),
      json!({ "tab": "automation", "ask": "rotate-token" }),
    ),
    (
      json!({ "route": "settings", "tab": "orchester", "ask": "sign-out" }),
      "/settings?tab=orchester&ask=sign-out".to_owned(),
      json!({ "tab": "orchester", "ask": "sign-out" }),
    ),
    (
      json!({ "route": "settings", "tab": "orchester", "ask": "decline", "invite": INVITE }),
      format!("/settings?tab=orchester&ask=decline&invite={INVITE}"),
      json!({ "tab": "orchester", "ask": "decline", "invite": INVITE }),
    ),
    (
      json!({ "route": "settings", "tab": "orchester", "server": SERVER, "ask": "sign-out" }),
      format!("/settings?tab=orchester&server={SERVER}&ask=sign-out"),
      json!({ "tab": "orchester", "server": SERVER, "ask": "sign-out" }),
    ),
    (
      json!({ "route": "settings", "tab": "connect", "server": SERVER, "step": "username" }),
      format!("/settings?tab=orchester&server={SERVER}&panel=connect&step=join"),
      json!({ "tab": "connect", "server": SERVER, "step": "join" }),
    ),
    (
      json!({ "route": "settings", "tab": "invite", "server": SERVER, "project": "billing" }),
      format!("/settings/invite?server={SERVER}&project=billing"),
      json!({ "tab": "invite", "server": SERVER, "project": "billing" }),
    ),
  ];
  for (args, path, params) in cases {
    let answer = navigate(&navigation, &args).expect("a valid route");
    assert_eq!(answer["route"]["path"], json!(path), "{args}");
    assert_eq!(answer["route"]["params"], params, "{args}");
    let mut again = params.as_object().expect("params are an object").clone();
    again.insert("route".to_owned(), json!("settings"));
    let repeated = navigate(&navigation, &Value::Object(again)).expect("its params name the same route");
    assert_eq!(repeated["route"]["path"], json!(path), "{args}");
    assert_eq!(Route::parse(&path).expect("the path reads back").path(), path, "{args}");
  }
}

#[test]
fn settings_navigate_refuses_what_a_tab_does_not_take() {
  let window = Window::open();
  let navigation = window.navigation();
  let refusals = [
    (
      json!({ "route": "settings", "tab": "connect", "step": "compare" }),
      "missing_param",
    ),
    (
      json!({ "route": "settings", "tab": "accounts", "panel": "remove" }),
      "missing_param",
    ),
    (
      json!({ "route": "settings", "tab": "orchester", "ask": "decline" }),
      "missing_param",
    ),
    (
      json!({ "route": "settings", "tab": "invite", "server": SERVER }),
      "missing_param",
    ),
    (json!({ "route": "settings", "ask": "sign-out" }), "invalid_param"),
    (
      json!({ "route": "settings", "tab": "orchester", "ask": "rotate-token" }),
      "invalid_param",
    ),
    (
      json!({ "route": "settings", "tab": "orchester", "server": SERVER, "ask": "decline", "invite": INVITE }),
      "invalid_param",
    ),
    (
      json!({ "route": "settings", "tab": "accounts", "panel": "edit" }),
      "invalid_param",
    ),
    (
      json!({ "route": "settings", "tab": "connect", "server": SERVER, "step": "done" }),
      "invalid_param",
    ),
    (
      json!({ "route": "settings", "tab": "accounts", "step": "join" }),
      "unexpected_param",
    ),
    (
      json!({ "route": "settings", "tab": "general", "server": SERVER }),
      "unexpected_param",
    ),
    (
      json!({ "route": "settings", "tab": "accounts", "account": "Work" }),
      "no_session",
    ),
  ];
  for (args, code) in refusals {
    let error = navigate(&navigation, &args).expect_err("refused");
    assert_eq!(error.code(), code, "{args}: {error}");
  }
}

#[test]
fn settings_navigate_finds_an_account_by_its_name() {
  let window = Window::open();
  let work = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::ClaudeCode,
    Name::new("Work").expect("a name"),
    AccountFolder::Created(window.data.dir.path().join("accounts").join("claude-work")),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&window.data.database())
    .expect("the database")
    .add_agent_account(&work)
    .expect("added");
  let lookup = Lookup::of(window.launch.services().expect("the session's services"));
  let navigation = window.navigation();
  navigate_with(
    &navigation,
    &json!({ "route": "settings", "tab": "accounts", "account": "work" }),
    Some(&lookup),
  )
  .expect("Work by its name, in any case");
  assert_eq!(
    navigation.route_now().path(),
    format!("/settings?tab=accounts&account={}", work.id())
  );
  let unknown = navigate_with(
    &navigation,
    &json!({ "route": "settings", "tab": "accounts", "account": "Lab" }),
    Some(&lookup),
  )
  .expect_err("no account Lab");
  assert_eq!(unknown.code(), "invalid_param");
}

#[test]
fn settings_navigate_routes_keep_their_step_and_question() {
  let server = SERVER.parse().expect("a server id");
  let invite = INVITE.parse().expect("an invite id");
  let views = [
    OrchesterView {
      server: Some(server),
      panel: OrchesterPanel::Connect,
      step: ConnectStep::Compare,
      ask: None,
      at: None,
    },
    OrchesterView {
      ask: Some(OrchesterAsk::Decline(invite)),
      ..OrchesterView::default()
    },
    OrchesterView {
      ask: Some(OrchesterAsk::SignOut),
      ..OrchesterView::server(server)
    },
    OrchesterView {
      panel: OrchesterPanel::Invites,
      at: Some(invite),
      ..OrchesterView::default()
    },
  ];
  for view in views {
    let route = Route::Orchester(view);
    assert_eq!(Route::parse(&route.path()), Ok(route.clone()), "{}", route.path());
  }
  assert_eq!(Route::parse("/settings?ask=rotate-token"), Ok(Route::RotateToken));
  assert!(
    Route::parse("/settings?ask=sign-out").is_err(),
    "Automation asks rotate-token only"
  );
  assert!(
    Route::parse(&format!("/settings?tab=orchester&invite={INVITE}")).is_err(),
    "an invite goes with ask=decline"
  );
}
