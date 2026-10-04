//! Settings › Servers (10.01–10.08) against a real `orchester-server` on loopback: adding a server
//! checks its address first, signing in with a first password asks for the person's own, the token
//! goes to the keychain (the harness's one in memory) and passwords are masked; a redirect is an
//! error naming its target; a token the server refuses shows the server as signed out with *Sign
//! in* in the status bar; sessions are revoked and a password change signs the others out; an admin
//! creates users, sees the first password once, resets, disables and enables them; Sign out forgets
//! the token, and the connection is checked when the application starts.

#[path = "../server_support/mod.rs"]
mod server_support;

use std::{
  cell::RefCell,
  io::{Read, Write},
  net::TcpListener,
  thread,
};

use orchester_core::secret::SecretValue;
use server_support::{SERVER_NAME, TestServer};

use crate::{agent_permissions::shows, harness::Desktop};

const TABS: usize = 12;
const OWN: &str = "alice's own long password";

/// Types `text` into the field `id`, as a user does.
fn type_into(desktop: &mut Desktop, id: &str, text: &str) {
  desktop.tree.get_element_by_id_mut(id).expect(id).focus();
  desktop.pass();
  desktop.type_text(text);
}

/// Deletes `typed` from the end of the field `id`, leaving the focus there.
fn clear(desktop: &mut Desktop, id: &str, typed: &str) {
  desktop.tree.get_element_by_id_mut(id).expect(id).focus();
  desktop.pass();
  for _ in typed.chars() {
    desktop.key("Backspace", "Backspace");
  }
}

fn servers_page(desktop: &mut Desktop) {
  desktop.wait_for("nav-app-settings");
  desktop.click("nav-app-settings");
  desktop.wait_for("add-server");
}

/// Adds the server at `url` through the dialog and waits for Sign in.
fn add_server(desktop: &mut Desktop, url: &str) {
  desktop.click("add-server");
  desktop.wait_for("add-server-address");
  type_into(desktop, "add-server-address", url);
  desktop.enter();
  desktop.wait_for("add-server-found");
  desktop.enter();
  desktop.wait_for("sign-in-username");
}

/// Signs `username` in with the first password `first` and chooses `own`.
fn sign_in_first_time(desktop: &mut Desktop, username: &str, first: &str, own: &str) {
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
  desktop.wait_for("status-server");
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
  let mut desktop = Desktop::with_project();
  servers_page(&mut desktop);
  add_server(&mut desktop, &server.url());
  sign_in_first_time(&mut desktop, "bohdan", bohdan.expose(), "bohdan's own long password");
  assert!(
    desktop.tree.get_element_by_id("server-tab-users").is_none(),
    "Users is for admins"
  );

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
  let (mut desktop, _server) = signed_in_as_admin();
  desktop.click("server-tab-users");
  desktop.wait_for("server-create-user");
  desktop.click("server-create-user");
  type_into(&mut desktop, "create-user-username", "maria");
  type_into(&mut desktop, "create-user-display-name", "Maria Lind");
  desktop.enter();
  desktop.wait_for("first-password-value");
  assert!(shows(&mut desktop, "Maria Lind can sign in"));
  let first = desktop
    .tree
    .get_element_by_id_mut("first-password-value")
    .and_then(|element| element.as_text_input())
    .expect("the first password");
  assert!(first.is_masked(), "the MCP and DevTools trees see its mask only");
  assert!(!first.value().is_empty(), "the server made a first password");
  desktop.click("first-password-done");
  desktop.wait_for("server-user-reset-maria");

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
  let (desktop, _server) = signed_in_as_admin();
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
