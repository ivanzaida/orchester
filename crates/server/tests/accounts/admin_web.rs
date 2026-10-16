//! Signing in to the server admin in a browser: setup, pairing, account and cloud sign-ins as `admin_web` device
//! tokens that live 12 hours, reach only what the admin pages call, and exist only for accounts with something to
//! manage. `GET /admin/sessions` lists the computers and `GET /admin/browsers` the browsers; `DELETE` signs one out.

use orchester_api::{
  AdminBrowsers, AdminSessions, Credential, ErrorCode, Joined, JoinedVia, PairingCode, ProjectCreated,
  cloud::TokenPurpose,
};
use serde_json::{Value, json};

use crate::{
  cloud::{bob, cloud_join, linked, refused},
  support::{Response, TestServer, credential},
};

const HOUR_MS: i64 = 60 * 60 * 1000;

async fn admin_code(server: &TestServer, code: &Credential, username: Option<&str>) -> Response {
  let mut body = json!({"code": code.expose(), "device": "Edge on Windows", "kind": "admin_web"});
  if let Some(username) = username {
    body["username"] = json!(username);
    body["display_name"] = json!("Ivan Zaida");
  }
  server.request("POST", "/auth/code", None, Some(body)).await
}

async fn project(server: &TestServer, token: &Credential, slug: &str) -> ProjectCreated {
  let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": slug, "slug": slug});
  let response = server.request("POST", "/projects", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

#[tokio::test]
async fn sa_setup_in_the_browser_checks_the_code_then_makes_the_first_admin() {
  let server = TestServer::start(|_| {}).await;
  let code = server.server.host_setup_code().await.expect("code").expect("no admin");
  let wrong = server
    .request("POST", "/auth/setup", None, Some(json!({"code": "ZZZZ-ZZZZ"})))
    .await;
  refused(&wrong, 401, ErrorCode::InvalidCode);
  let checked = server
    .request("POST", "/auth/setup", None, Some(json!({"code": code.expose()})))
    .await;
  assert_eq!(checked.status, 204, "checking uses nothing up");
  let checked_again = server
    .request(
      "POST",
      "/auth/setup",
      None,
      Some(json!({"code": code.expose().to_lowercase()})),
    )
    .await;
  assert_eq!(checked_again.status, 204);
  let joined = admin_code(&server, &code, Some("ivan")).await;
  assert_eq!(joined.status, 201);
  let joined: Joined = joined.json();
  assert!(joined.user.admin);
  assert_eq!(joined.session.via, JoinedVia::Code);
  let after = server
    .request("POST", "/auth/setup", None, Some(json!({"code": code.expose()})))
    .await;
  refused(&after, 401, ErrorCode::InvalidCode);
  let info: Value = server.request("GET", "/server", None, None).await.json();
  assert_eq!(info["needs_setup"], json!(false));
  server.finish().await;
}

#[tokio::test]
async fn sa_an_admin_sign_in_reaches_only_the_admin_and_ends_after_12_hours() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let kontur = project(&server, &admin.token, "kontur").await;
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&admin.token), None)
    .await
    .json();
  let browser: Joined = admin_code(&server, &pairing.code, None).await.json();
  let token = &browser.token;
  for path in [
    "/auth/me",
    "/admin/users",
    "/admin/projects",
    "/admin/sessions",
    "/server/settings",
  ] {
    assert_eq!(
      server.request("GET", path, Some(token), None).await.status,
      200,
      "{path}"
    );
  }
  let tasks = format!("/projects/{}/tasks", kontur.project.id);
  let events = format!("/projects/{}/events", kontur.project.id);
  for (method, path) in [
    ("GET", tasks.as_str()),
    ("GET", events.as_str()),
    ("POST", "/auth/pairings"),
    ("GET", "/auth/sessions"),
  ] {
    let response = server.request(method, path, Some(token), None).await;
    refused(&response, 403, ErrorCode::Forbidden);
  }
  server.clock.advance(11 * HOUR_MS);
  assert_eq!(server.request("GET", "/auth/me", Some(token), None).await.status, 200);
  server.clock.advance(HOUR_MS + 1);
  refused(
    &server.request("GET", "/auth/me", Some(token), None).await,
    401,
    ErrorCode::Unauthorized,
  );
  assert_eq!(
    server.request("GET", "/auth/me", Some(&admin.token), None).await.status,
    200,
    "the app's own token lives on"
  );
  server.finish().await;
}

#[tokio::test]
async fn sa_nobody_signs_in_to_the_admin_without_something_to_manage() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let member = server.invited("bob", false).await;
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&member.token), None)
    .await
    .json();
  refused(
    &admin_code(&server, &pairing.code, None).await,
    403,
    ErrorCode::NothingToManage,
  );
  let kontur = project(&server, &admin.token, "kontur").await;
  let owner = json!({"username": "bob", "role": "owner"});
  let path = format!("/projects/{}/members", kontur.project.id);
  assert_eq!(
    server
      .request("POST", &path, Some(&admin.token), Some(owner))
      .await
      .status,
    201
  );
  let browser = admin_code(&server, &pairing.code, None).await;
  assert_eq!(browser.status, 201, "the refused sign-in did not use the code up");
  let browser: Joined = browser.json();
  let projects: Value = server
    .request("GET", "/admin/projects", Some(&browser.token), None)
    .await
    .json();
  assert_eq!(projects["projects"].as_array().map(Vec::len), Some(1));
  let invite: Value = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code","admin":false})),
    )
    .await
    .json();
  let invite_code = credential(invite["code"].as_str().expect("code"));
  refused(
    &admin_code(&server, &invite_code, Some("carol")).await,
    401,
    ErrorCode::InvalidCode,
  );
  assert_eq!(server.redeem(&invite_code, Some("carol"), "laptop").await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn sa_a_cloud_sign_in_to_the_admin_needs_a_token_issued_for_it() {
  let (server, admin, cloud, audience) = linked().await;
  let link: Value = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link":"start"})),
    )
    .await
    .json();
  assert!(link["statement"].is_string());
  let identity = bob(&audience, None);
  let finish = json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds()),
    "identity_token": cloud.purposed_token(&identity, TokenPurpose::Admin, server.seconds())}});
  let wrong_purpose = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(finish))
    .await;
  refused(&wrong_purpose, 401, ErrorCode::InvalidIdentity);
  let finish = json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds()),
    "identity_token": cloud.purposed_token(&identity, TokenPurpose::Link, server.seconds())}});
  let finished: Value = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(finish))
    .await
    .json();
  assert_eq!(finished["cloud"]["linked_by"]["display_name"], json!("Alice"));
  let sign_in = |token: String, kind: &str| json!({"identity_token": token, "device": "Edge on Windows", "kind": kind});
  let plain = cloud.token(&identity, server.seconds());
  let response = server
    .request("POST", "/auth/cloud", None, Some(sign_in(plain, "admin_web")))
    .await;
  refused(&response, 401, ErrorCode::InvalidIdentity);
  let for_admin = cloud.purposed_token(&identity, TokenPurpose::Admin, server.seconds());
  refused(
    &cloud_join(&server, &for_admin, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let for_admin = cloud.purposed_token(&identity, TokenPurpose::Admin, server.seconds());
  let browser = server
    .request("POST", "/auth/cloud", None, Some(sign_in(for_admin, "admin_web")))
    .await;
  assert_eq!(browser.status, 201);
  let browser: Joined = browser.json();
  assert!(browser.user.admin && browser.user.cloud);
  let stranger = bob(&audience, None);
  let stranger = crate::support::cloud::Identity {
    subject: "0191c7c0-0000-7000-8000-00000000c0c0",
    email: "carol@example.com",
    ..stranger
  };
  let token = cloud.purposed_token(&stranger, TokenPurpose::Admin, server.seconds());
  refused(
    &server
      .request("POST", "/auth/cloud", None, Some(sign_in(token, "admin_web")))
      .await,
    403,
    ErrorCode::NotInvited,
  );
  server.finish().await;
}

#[tokio::test]
async fn sa_admins_list_every_computer_and_sign_any_one_out() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&admin.token), None)
    .await
    .json();
  let browser: Joined = admin_code(&server, &pairing.code, None).await.json();
  server.clock.advance(5 * 60 * 1000);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&bob.token), None).await.status,
    200
  );
  let listed: AdminSessions = server
    .request("GET", "/admin/sessions", Some(&browser.token), None)
    .await
    .json();
  let devices: Vec<(&str, &str)> = listed
    .sessions
    .iter()
    .map(|session| (session.user.username.as_str(), session.device.as_str()))
    .collect();
  assert_eq!(
    devices,
    [("bob", "test computer"), ("alice", "test computer")],
    "newest use first, no browser"
  );
  let own: orchester_api::SessionsResponse = server
    .request("GET", "/auth/sessions", Some(&admin.token), None)
    .await
    .json();
  assert_eq!(
    own.sessions.len(),
    1,
    "an account's own computers leave its browser sign-ins out too"
  );
  let bobs = &listed.sessions[0];
  assert!(!bobs.current);
  assert_eq!(bobs.expires_at, bobs.last_used_at + 90 * 24 * HOUR_MS);
  refused(
    &server.request("GET", "/admin/sessions", Some(&bob.token), None).await,
    403,
    ErrorCode::Forbidden,
  );
  let path = format!("/admin/sessions/{}", bobs.id);
  assert_eq!(
    server.request("DELETE", &path, Some(&browser.token), None).await.status,
    204
  );
  refused(
    &server.request("GET", "/auth/me", Some(&bob.token), None).await,
    401,
    ErrorCode::Unauthorized,
  );
  refused(
    &server.request("DELETE", &path, Some(&browser.token), None).await,
    404,
    ErrorCode::NotFound,
  );
  let browser_path = format!("/admin/sessions/{}", browser.session.id);
  refused(
    &server
      .request("DELETE", &browser_path, Some(&browser.token), None)
      .await,
    404,
    ErrorCode::NotFound,
  );
  server.finish().await;
}

/// Security review 1, minor 5: browsers signed in to the admin are listed apart from the computers, and an admin signs
/// any one of them out.
#[tokio::test]
async fn sa_admins_list_the_browsers_signed_in_to_the_admin_and_sign_one_out() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let first = browser_with_pairing(&server, &admin.token).await;
  server.clock.advance(5 * 60 * 1000);
  let second = browser_with_pairing(&server, &admin.token).await;
  let listed: AdminBrowsers = server
    .request("GET", "/admin/browsers", Some(&first.token), None)
    .await
    .json();
  let ids: Vec<_> = listed
    .browsers
    .iter()
    .map(|browser| (browser.id, browser.current))
    .collect();
  assert_eq!(
    ids,
    [(second.session.id, false), (first.session.id, true)],
    "newest sign-in first"
  );
  let newest = &listed.browsers[0];
  assert_eq!((newest.user.username.as_str(), newest.via), ("alice", JoinedVia::Code));
  assert_eq!(newest.expires_at, newest.created_at + 12 * HOUR_MS);
  refused(
    &server.request("GET", "/admin/browsers", Some(&bob.token), None).await,
    403,
    ErrorCode::Forbidden,
  );
  let computer = format!("/admin/browsers/{}", admin.session.id);
  refused(
    &server.request("DELETE", &computer, Some(&first.token), None).await,
    404,
    ErrorCode::NotFound,
  );
  let path = format!("/admin/browsers/{}", second.session.id);
  assert_eq!(
    server.request("DELETE", &path, Some(&first.token), None).await.status,
    204
  );
  refused(
    &server.request("GET", "/auth/me", Some(&second.token), None).await,
    401,
    ErrorCode::Unauthorized,
  );
  refused(
    &server.request("DELETE", &path, Some(&first.token), None).await,
    404,
    ErrorCode::NotFound,
  );
  let left: AdminBrowsers = server
    .request("GET", "/admin/browsers", Some(&first.token), None)
    .await
    .json();
  assert_eq!(left.browsers.len(), 1);
  server.finish().await;
}

/// Links the first admin's account to `bob`'s cloud identity, as linking the server through `/authorize` does.
async fn link_admin_to_bob(
  server: &TestServer,
  admin: &Joined,
  cloud: &crate::support::cloud::TestCloud,
  audience: &str,
) {
  let started = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link":"start"})),
    )
    .await;
  assert_eq!(started.status, 200);
  let token = cloud.purposed_token(&bob(audience, None), TokenPurpose::Link, server.seconds());
  let finish = json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds()),
    "identity_token": token}});
  let finished = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(finish))
    .await;
  assert_eq!(finished.status, 200);
}

async fn browser_with_pairing(server: &TestServer, token: &Credential) -> Joined {
  let pairing: PairingCode = server.request("POST", "/auth/pairings", Some(token), None).await.json();
  let joined = admin_code(server, &pairing.code, None).await;
  assert_eq!(joined.status, 201);
  joined.json()
}

/// Security review 1, B2: while join approval is on, a cloud identity alone does not open the admin; a code from one
/// of the account's own computers does.
#[tokio::test]
async fn sa_with_join_approval_on_a_cloud_sign_in_to_the_admin_is_refused() {
  let (server, admin, cloud, audience) = linked().await;
  link_admin_to_bob(&server, &admin, &cloud, &audience).await;
  let approval = json!({"join_approval": true});
  assert_eq!(
    server
      .request("PATCH", "/server/settings", Some(&admin.token), Some(approval))
      .await
      .status,
    200
  );
  let token = cloud.purposed_token(&bob(&audience, None), TokenPurpose::Admin, server.seconds());
  let body = json!({"identity_token": token, "device": "Edge on Windows", "kind": "admin_web"});
  let response = server.request("POST", "/auth/cloud", None, Some(body.clone())).await;
  refused(&response, 403, ErrorCode::ApprovalRequired);
  let approvals: Value = server
    .request("GET", "/auth/approvals", Some(&admin.token), None)
    .await
    .json();
  assert_eq!(
    approvals["approvals"].as_array().map(Vec::len),
    Some(0),
    "nothing waits: the sign-in is refused"
  );
  browser_with_pairing(&server, &admin.token).await;
  let off = json!({"join_approval": false});
  assert_eq!(
    server
      .request("PATCH", "/server/settings", Some(&admin.token), Some(off))
      .await
      .status,
    200
  );
  assert_eq!(
    server.request("POST", "/auth/cloud", None, Some(body)).await.status,
    201,
    "the refused token was not used up"
  );
  server.finish().await;
}

/// Security review 1, B2: a browser signed in to the admin never answers its own account's waiting joins.
#[tokio::test]
async fn sa_an_admin_browser_never_answers_its_own_accounts_approvals() {
  let (server, admin, cloud, audience) = linked().await;
  link_admin_to_bob(&server, &admin, &cloud, &audience).await;
  let approval = json!({"join_approval": true});
  assert_eq!(
    server
      .request("PATCH", "/server/settings", Some(&admin.token), Some(approval))
      .await
      .status,
    200
  );
  let waiting = cloud_join(&server, &cloud.token(&bob(&audience, None), server.seconds()), None).await;
  assert_eq!(waiting.status, 202);
  let id = waiting.json::<Value>()["approval"]["id"]
    .as_str()
    .expect("an approval")
    .to_owned();
  let browser = browser_with_pairing(&server, &admin.token).await;
  let listed: Value = server
    .request("GET", "/auth/approvals", Some(&browser.token), None)
    .await
    .json();
  assert_eq!(
    listed["approvals"].as_array().map(Vec::len),
    Some(0),
    "not even listed to its own browser"
  );
  let path = format!("/auth/approvals/{id}");
  let answered = server
    .request("POST", &path, Some(&browser.token), Some(json!({"approve": true})))
    .await;
  refused(&answered, 404, ErrorCode::NotFound);
  let other = server.invited("carol", true).await;
  let other_browser = browser_with_pairing(&server, &other.token).await;
  let listed: Value = server
    .request("GET", "/auth/approvals", Some(&other_browser.token), None)
    .await
    .json();
  assert_eq!(
    listed["approvals"].as_array().map(Vec::len),
    Some(1),
    "another admin's browser sees it"
  );
  let own_computer = server
    .request("POST", &path, Some(&admin.token), Some(json!({"approve": true})))
    .await;
  assert_eq!(
    own_computer.status, 200,
    "one of the account's own computers answers it"
  );
  server.finish().await;
}
