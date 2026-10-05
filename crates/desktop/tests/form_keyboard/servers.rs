//! Settings › Servers (10.01–10.08) against a real `orchester-server` on loopback: adding a server
//! checks its address first, signing in with a first password asks for the person's own, the token
//! goes to the keychain (the harness's one in memory) and passwords are masked; a redirect is an
//! error naming its target; a token the server refuses shows the server as signed out with *Sign
//! in* in the status bar; sessions are revoked and a password change signs the others out; an admin
//! creates users, sees the first password once (painted in clear, `•••` in the MCP tree), resets,
//! disables and enables them; Sign out forgets the token, and the connection is checked when the
//! application starts.

#[path = "../server_support/mod.rs"]
pub(crate) mod server_support;

use std::{
  cell::RefCell,
  io::{Read, Write},
  net::TcpListener,
  thread,
};

use lurq::{
  clipboard::{copy_to_clipboard, read_from_clipboard},
  core::REDACTED,
  layout::quad::QuadContent,
};
use orchester_core::secret::SecretValue;
use serde_json::json;
use server_support::{SERVER_NAME, TestServer};

use crate::{agent_permissions::shows, harness::Desktop};

const TABS: usize = 12;
const OWN: &str = "alice's own long password";

/// Types `text` into the field `id`, as a user does.
pub(crate) fn type_into(desktop: &mut Desktop, id: &str, text: &str) {
  desktop.tree.get_element_by_id_mut(id).expect(id).focus();
  desktop.pass();
  desktop.type_text(text);
}

/// Deletes `typed` from the end of the field `id`, leaving the focus there.
pub(crate) fn clear(desktop: &mut Desktop, id: &str, typed: &str) {
  desktop.tree.get_element_by_id_mut(id).expect(id).focus();
  desktop.pass();
  for _ in typed.chars() {
    desktop.key("Backspace", "Backspace");
  }
}

pub(crate) fn servers_page(desktop: &mut Desktop) {
  desktop.wait_for("nav-app-settings");
  desktop.click("nav-app-settings");
  desktop.wait_for("add-server");
}

/// Adds the server at `url` through the dialog and waits for Sign in.
pub(crate) fn add_server(desktop: &mut Desktop, url: &str) {
  desktop.click("add-server");
  desktop.wait_for("add-server-address");
  type_into(desktop, "add-server-address", url);
  desktop.enter();
  desktop.wait_for("add-server-found");
  desktop.enter();
  desktop.wait_for("sign-in-username");
}

/// Signs `username` in with the first password `first` and chooses `own`.
pub(crate) fn sign_in_first_time(desktop: &mut Desktop, username: &str, first: &str, own: &str) {
  type_into(desktop, "sign-in-username", username);
  type_into(desktop, "sign-in-password", first);
  desktop.enter();
  desktop.wait_for("choose-password-new");
  type_into(desktop, "choose-password-new", own);
  type_into(desktop, "choose-password-repeat", own);
  desktop.enter();
  desktop.settle("the signed-in account", |tree| {
    tree.get_element_by_id("choose-password").is_none() && tree.get_element_by_id("server-sessions").is_some()
  });
}

/// A desktop signed in to a fresh server as `alice`, its first admin.
fn signed_in_as_admin() -> (Desktop, TestServer) {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let mut desktop = Desktop::with_project();
  servers_page(&mut desktop);
  add_server(&mut desktop, &server.url());
  sign_in_first_time(&mut desktop, "alice", first.expose(), OWN);
  (desktop, server)
}

/// The token this computer keeps for the server.
fn token(desktop: &Desktop) -> SecretValue {
  let values = desktop.vault.values();
  assert_eq!(values.len(), 1, "one token in the keychain");
  SecretValue::new(values[0].clone()).expect("a token")
}

/// The id of the first session's *Revoke*.
fn revoke_button(desktop: &mut Desktop) -> Option<String> {
  let found = RefCell::new(None);
  desktop.tree.find_element(|element| {
    if let Some(id) = element.id().filter(|id| id.starts_with("server-revoke-")) {
      found.borrow_mut().get_or_insert_with(|| id.to_owned());
    }
    false
  });
  found.into_inner()
}

/// Creates the user `maria` in Users and waits for her first password (10.08 B).
fn create_maria(desktop: &mut Desktop) {
  desktop.click("server-tab-users");
  desktop.wait_for("server-create-user");
  desktop.click("server-create-user");
  type_into(desktop, "create-user-username", "maria");
  type_into(desktop, "create-user-display-name", "Maria Lind");
  desktop.enter();
  desktop.wait_for("first-password-value");
}

/// The first password the dialog shows, as the app holds it.
fn first_password(desktop: &mut Desktop) -> String {
  desktop
    .tree
    .get_element_by_id_mut("first-password-value")
    .and_then(|element| element.text_content().map(str::to_owned))
    .expect("the first password's text")
}

/// The first password is painted in clear, as the design draws it, while the MCP tree shows only
/// `•••` in its place. Returns the password.
fn first_password_shown_only_on_screen(desktop: &mut Desktop) -> String {
  let password = first_password(desktop);
  assert!(
    !password.is_empty() && password != REDACTED,
    "the server made a first password"
  );
  let painted = desktop
    .tree
    .painted_quads()
    .into_iter()
    .any(|quad| matches!(quad.content, QuadContent::Text { text, .. } if text == password));
  assert!(painted, "the first password is painted in clear");
  let outputs = desktop.with_mcp_agent(|mut agent| {
    [
      agent.tool("lurq_read_tree", &json!({})),
      agent.tool("lurq_find_by_id", &json!({"id": "first-password-value"})),
      agent.tool("lurq_inspect", &json!({"query": "first-password"})),
    ]
  });
  for output in &outputs {
    assert!(!output.contains(&password), "the MCP tree holds the password");
    assert!(output.contains(REDACTED), "{output}");
  }
  assert!(
    outputs[0]
      .lines()
      .any(|line| line.contains("#first-password-value") && line.contains("sensitive=true")),
    "{}",
    outputs[0]
  );
  password
}

/// Leaves the page and comes back, so it reads the server again.
fn reopen(desktop: &mut Desktop) {
  desktop.click("nav-tasks");
  desktop.wait_for("nav-app-settings");
  desktop.click("nav-app-settings");
  desktop.wait_for("server-details");
}

#[test]
fn servers_add_checks_the_address_and_the_first_sign_in_asks_for_an_own_password() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let mut desktop = Desktop::with_project();
  servers_page(&mut desktop);
  desktop.wait_for("servers-empty");
  desktop.click("add-server");
  desktop.tab_to("add-server-address", TABS);
  desktop.type_text(&server.url());
  desktop.enter();
  desktop.wait_for("add-server-found");
  assert!(shows(&mut desktop, SERVER_NAME));
  assert!(
    shows(&mut desktop, "nothing leaves it"),
    "plain HTTP on loopback is allowed"
  );
  desktop.enter();
  desktop.wait_for("sign-in-username");

  type_into(&mut desktop, "sign-in-username", "alice");
  type_into(&mut desktop, "sign-in-password", "not the password");
  let password = desktop
    .tree
    .get_element_by_id_mut("sign-in-password")
    .and_then(|element| element.as_text_input())
    .expect("the password field");
  assert!(password.is_masked(), "masked on screen and in the MCP tree");
  desktop.enter();
  desktop.wait_for("sign-in-error");
  assert!(desktop.vault.values().is_empty());

  clear(&mut desktop, "sign-in-password", "not the password");
  desktop.type_text(first.expose());
  desktop.enter();
  desktop.wait_for("choose-password-new");
  assert!(
    desktop.vault.values().is_empty(),
    "a first-password session is not kept"
  );
  type_into(&mut desktop, "choose-password-new", "too short");
  desktop.wait_for("choose-new-error");
  clear(&mut desktop, "choose-password-new", "too short");
  desktop.type_text(OWN);
  type_into(&mut desktop, "choose-password-repeat", OWN);
  desktop.enter();
  desktop.settle("the signed-in account", |tree| {
    tree.get_element_by_id("server-sessions").is_some()
  });

  assert!(token(&desktop).expose().starts_with("orc_"));
  assert!(server.accepts(&token(&desktop)));
  assert!(!shows(&mut desktop, OWN) && !shows(&mut desktop, first.expose()));
  // 10.43 G: no shared project is open, so the status bar has no server item.
  assert!(desktop.tree.get_element_by_id("status-server").is_none());
}

#[test]
fn servers_redirect_is_an_error_that_names_its_target() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
  let port = listener.local_addr().expect("address").port();
  thread::spawn(move || {
    for mut stream in listener.incoming().flatten() {
      let mut request = [0_u8; 4096];
      // What was asked does not matter; the answer is the same.
      let _ = stream.read(&mut request);
      let _ = stream.write_all(
        b"HTTP/1.1 302 Found\r\nLocation: https://sso.kontur.lan/login\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
      );
    }
  });
  let mut desktop = Desktop::with_project();
  servers_page(&mut desktop);
  desktop.click("add-server");
  type_into(&mut desktop, "add-server-address", &format!("http://127.0.0.1:{port}"));
  desktop.enter();
  desktop.wait_for("add-server-error");
  assert!(shows(&mut desktop, "https://sso.kontur.lan/login"));
  assert!(desktop.tree.get_element_by_id("add-server-found").is_none());
}

#[test]
fn servers_a_refused_token_shows_signed_out_and_the_status_bar_signs_in_again() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let admin = server.sign_in_elsewhere("alice", first.expose(), OWN);
  let bohdan = server.create_user("bohdan", "Bohdan Koval", false);
  let mut desktop = crate::sharing::signed_in_on(&server, ("bohdan", &bohdan), &[]);
  assert!(
    desktop.tree.get_element_by_id("server-tab-users").is_none(),
    "Users is for admins"
  );
  // The status bar shows the servers of the open shared projects.
  crate::sharing::share(&mut desktop);
  desktop.wait_for("status-server");

  server.reset_password(&admin, "bohdan");
  reopen(&mut desktop);
  desktop.wait_for("server-signed-out");
  assert!(desktop.vault.values().is_empty(), "the refused token left the keychain");
  desktop.wait_for("status-server-sign-in");
  desktop.click("status-server-sign-in");
  desktop.wait_for("sign-in-username");
}

#[test]
fn servers_revoke_and_a_password_change_sign_the_other_computers_out() {
  let (mut desktop, server) = signed_in_as_admin();
  let laptop = server.sign_in_elsewhere("alice", OWN, OWN);
  reopen(&mut desktop);
  desktop.wait_until("the other session", |desktop| revoke_button(desktop).is_some());
  let revoke = revoke_button(&mut desktop).expect("Revoke");
  desktop.click(&revoke);
  desktop.wait_until("the revoked session", |_| !server.accepts(&laptop));

  let build = server.sign_in_elsewhere("alice", OWN, OWN);
  desktop.click("server-change-password");
  desktop.wait_for("change-password-current");
  let new = "alice's newer long password";
  type_into(&mut desktop, "change-password-current", OWN);
  type_into(&mut desktop, "change-password-new", new);
  type_into(&mut desktop, "change-password-repeat", new);
  desktop.enter();
  desktop.settle("the dialog closes", |tree| {
    tree.get_element_by_id("change-password").is_none()
  });
  assert!(!server.accepts(&build), "the other session is signed out");
  assert!(server.accepts(&token(&desktop)), "this computer stays signed in");
}

#[test]
fn servers_admin_creates_resets_disables_and_enables_users() {
  let (mut desktop, server) = signed_in_as_admin();
  create_maria(&mut desktop);
  assert!(shows(&mut desktop, "Maria Lind can sign in"));
  let first = first_password_shown_only_on_screen(&mut desktop);
  desktop.click("first-password-done");
  desktop.wait_for("server-user-reset-maria");
  server.sign_in_elsewhere("maria", &first, "maria's own long password");

  desktop.click("server-create-user");
  type_into(&mut desktop, "create-user-username", "MARIA");
  type_into(&mut desktop, "create-user-display-name", "Maria Again");
  desktop.enter();
  desktop.wait_for("create-user-taken");
  desktop.click("create-user-cancel");

  desktop.click("server-user-reset-maria");
  desktop.click("reset-password-confirm");
  desktop.wait_for("first-password-value");
  desktop.click("first-password-done");

  desktop.wait_for("server-user-disable-maria");
  desktop.click("server-user-disable-maria");
  desktop.wait_for("disable-user-consequences");
  desktop.click("disable-user-confirm");
  desktop.wait_for("server-user-enable-maria");
  desktop.click("server-user-enable-maria");
  desktop.wait_for("server-user-disable-maria");
}

/// At 960 the details' header gives way from its name, then its label: *Sign out* stays whole.
#[test]
fn servers_sign_out_stays_whole_in_a_960_window() {
  let (mut desktop, _server) = signed_in_as_admin();
  let names: Vec<(String, f32)> = ids_with_prefix(&mut desktop, "server-card-name-")
    .into_iter()
    .map(|id| {
      let width = desktop.bounds(&id).width;
      (id, width)
    })
    .collect();
  assert!(!names.is_empty(), "the server cards are listed");
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.pass();
  let (button, header) = (
    desktop.bounds("server-sign-out"),
    desktop.bounds("server-details-header"),
  );
  assert!(button.width > 0.0, "Sign out is shown");
  assert!(
    button.x + button.width <= header.x + header.width + 0.5,
    "{button:?} inside {header:?}"
  );
  assert!(shows(&mut desktop, "Sign out"));
  // The rest of the details fit beside the list too: the account facts, the sessions table with
  // room for each device, and the keychain note.
  let details = desktop.bounds("server-details");
  let right = details.x + details.width + 0.5;
  for id in ["server-account-facts", "server-sessions", "server-token-note"] {
    let found = desktop.bounds(id);
    assert!(
      found.width > 0.0 && found.x + found.width <= right,
      "{id} {found:?} inside {details:?}"
    );
  }
  let devices = ids_with_prefix(&mut desktop, "server-session-device-");
  assert!(!devices.is_empty(), "the sessions are listed");
  for id in devices {
    let device = desktop.bounds(&id);
    assert!(device.width >= 120.0, "{id} keeps room for its name: {device:?}");
  }
  assert!(shows(&mut desktop, "Alice Moreau (alice)"));
  // The server card keeps its name as whole as in a wide window, beside the connection's icon.
  for (id, wide) in names {
    let name = desktop.bounds(&id);
    assert!(name.width + 0.5 >= wide, "{id} keeps its name: {name:?}, {wide} wide");
  }
  desktop.click("server-tab-users");
  desktop.wait_for("server-user-table");
  let users = desktop.bounds("server-user-table");
  assert!(
    users.x + users.width <= right,
    "the users table {users:?} inside {details:?}"
  );
}

#[test]
#[ignore = "requires an available text clipboard; run separately so the user's clipboard can be restored"]
fn servers_first_password_copy_puts_the_shown_password_on_the_clipboard() {
  let previous = read_from_clipboard().expect("a text clipboard whose previous contents can be restored");
  let (mut desktop, _server) = signed_in_as_admin();
  create_maria(&mut desktop);
  let shown = first_password(&mut desktop);
  desktop.click("first-password-copy");
  let copied = read_from_clipboard();
  assert!(copy_to_clipboard(&previous), "restore the previous clipboard text");
  assert!(
    copied.as_deref() == Some(shown.as_str()),
    "Copy puts the shown password on the clipboard"
  );
  assert!(shows(&mut desktop, "Copied"));
}

#[test]
fn servers_sign_out_forgets_the_token_and_the_account() {
  let (mut desktop, server) = signed_in_as_admin();
  let kept = token(&desktop);
  desktop.click("server-sign-out");
  desktop.wait_for("server-sign-in");
  assert!(desktop.vault.values().is_empty());
  assert!(!server.accepts(&kept), "the server ended the session");
  desktop.settle("no server in use", |tree| {
    tree.get_element_by_id("status-server").is_none()
  });
}

#[test]
fn servers_the_connection_is_checked_when_the_application_starts() {
  let (mut desktop, _server) = crate::sharing::signed_in("alice", true, &[]);
  crate::sharing::share(&mut desktop);
  // The harness's keychain is in memory: after the restart it has no token, as when the token was
  // removed from the keychain, and the check at start shows the server as signed out.
  let mut desktop = desktop.crash_and_restart(|| {});
  desktop.wait_for("status-server-sign-in");
}

/// 10.03 B: plain HTTP to a host that is not this computer by name. Windows resolves every
/// `*.localhost` name to the loopback address, so `lab.localhost` reaches the test server while the
/// desktop, which counts only `localhost` itself as this computer, treats it as another computer.
#[cfg(windows)]
#[test]
fn servers_plain_http_to_another_computer_needs_the_opt_in_under_its_warning() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let mut desktop = Desktop::with_project();
  servers_page(&mut desktop);
  desktop.click("add-server");
  type_into(
    &mut desktop,
    "add-server-address",
    &format!("http://lab.localhost:{}", server.address.port()),
  );
  desktop.click("add-server-continue");
  assert!(
    desktop.tree.get_element_by_id("add-server-checking").is_none(),
    "Continue does nothing before the server answered"
  );
  desktop.click("add-server-check");
  desktop.wait_for("add-server-plain-http");
  assert!(shows(&mut desktop, "Allow plain HTTP for lab.localhost:"));
  desktop.click("add-server-continue");
  assert!(
    desktop.tree.get_element_by_id("sign-in-username").is_none(),
    "Continue waits for the opt-in"
  );

  desktop.click("add-server-allow-http");
  desktop.click("add-server-continue");
  desktop.wait_for("sign-in-username");
  sign_in_first_time(&mut desktop, "alice", first.expose(), OWN);
  assert!(
    server.accepts(&token(&desktop)),
    "signed in over plain HTTP with the opt-in"
  );
}

/// The ids in the tree that start with `prefix`.
fn ids_with_prefix(desktop: &mut Desktop, prefix: &str) -> Vec<String> {
  let found = std::cell::RefCell::new(Vec::new());
  desktop.tree.find_element(|element| {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
      found.borrow_mut().push(id.to_owned());
    }
    false
  });
  found.into_inner()
}

/// The request that reads the person's sessions, and the one that reads the server's users.
const SESSIONS: &str = "GET /api/v1/auth/sessions ";
const USERS: &str = "GET /api/v1/admin/users ";

/// Lets the page run for 3 seconds, as a person who only looks at it.
fn stay(desktop: &mut Desktop) {
  let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
  while std::time::Instant::now() < until {
    desktop.tree.tick_futures();
    desktop.pass();
    std::thread::sleep(std::time::Duration::from_millis(50));
  }
}

fn shows_a_session(desktop: &mut Desktop) -> bool {
  let found = RefCell::new(false);
  desktop.tree.find_element(|element| {
    if element.id().is_some_and(|id| id.starts_with("server-session-device-")) {
      *found.borrow_mut() = true;
    }
    false
  });
  found.into_inner()
}

/// Settings › Servers reads the sessions and, for an admin, the users once for a visit, however
/// often the server answers meanwhile; opening the Users tab shows what was read.
#[test]
fn servers_ask_once_while_the_page_is_open() {
  let server = server_support::TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let count = crate::request_count::RequestCount::start(server.address, &[SESSIONS, USERS]);
  let mut desktop = crate::sharing::signed_in_via(&count.url(), ("alice", &first), &[]);
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  let (sessions, users) = (count.count(SESSIONS), count.count(USERS));
  servers_page(&mut desktop);
  desktop.wait_until("the sessions show", shows_a_session);
  stay(&mut desktop);
  assert!(shows_a_session(&mut desktop), "the sessions stay");
  assert_eq!(
    count.count(SESSIONS),
    sessions + 1,
    "one read of the sessions for the visit"
  );
  assert_eq!(count.count(USERS), users + 1, "one read of the users for the visit");
  desktop.click("server-tab-users");
  desktop.wait_for("server-create-user");
  stay(&mut desktop);
  assert!(shows(&mut desktop, "Alice Moreau"));
  assert_eq!(count.count(USERS), users + 1, "the Users tab shows what was read");
}
