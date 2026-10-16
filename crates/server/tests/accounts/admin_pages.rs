//! What the server admin's pages read and change: the server's settings and certificate, the projects an admin or
//! owner manages, members by username, the repository hint, and invites with their recent history.

use orchester_api::{
  ErrorCode, InviteRevoked, InviteState, Invites, ManagedProjects, Members, ProjectCreated, ServerInfo,
  SettingsResponse,
};
use orchester_server::config::TlsMode;
use serde_json::{Value, json};

use crate::{cloud::refused, support::TestServer};

async fn project(
  server: &TestServer,
  token: &orchester_api::Credential,
  slug: &str,
  hint: Option<&str>,
) -> ProjectCreated {
  let mut body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": "Kontur team app",
    "slug": slug});
  if let Some(hint) = hint {
    body["remote_hint"] = json!(hint);
  }
  let response = server.request("POST", "/projects", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

#[tokio::test]
async fn sa_settings_rename_the_server_and_toggle_join_approval() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let member = server.invited("bob", false).await;
  let shown: SettingsResponse = server
    .request("GET", "/server/settings", Some(&admin.token), None)
    .await
    .json();
  assert_eq!(shown.settings.name, "Orchester");
  assert!(!shown.settings.join_approval);
  assert!(
    shown.certificate.is_none(),
    "plain HTTP behind a proxy has no certificate of its own"
  );
  let changed = server
    .request(
      "PATCH",
      "/server/settings",
      Some(&admin.token),
      Some(json!({"name": "  Kontur team ", "join_approval": true})),
    )
    .await;
  assert_eq!(changed.status, 200);
  let changed: SettingsResponse = changed.json();
  assert_eq!(changed.settings.name, "Kontur team");
  assert!(changed.settings.join_approval);
  let info: ServerInfo = server.request("GET", "/server", None, None).await.json();
  assert_eq!(info.name, "Kontur team");
  assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
  for name in ["", "   ", &"x".repeat(129)] {
    let response = server
      .request(
        "PATCH",
        "/server/settings",
        Some(&admin.token),
        Some(json!({"name": name})),
      )
      .await;
    refused(&response, 422, ErrorCode::Invalid);
  }
  let response = server
    .request(
      "PATCH",
      "/server/settings",
      Some(&member.token),
      Some(json!({"join_approval": false})),
    )
    .await;
  refused(&response, 403, ErrorCode::Forbidden);
  server.finish().await;
}

#[tokio::test]
async fn sa_the_certificate_is_shown_for_comparing_in_a_browser() {
  let server = TestServer::start(|config| config.tls = TlsMode::SelfSigned).await;
  let info = server.tls_pinned_request("GET", "/server", None, None).await;
  let info: ServerInfo = info.json();
  let certificate = info.certificate.expect("its own certificate");
  assert_eq!(Some(certificate.check_code.clone()), server.server.check_code());
  assert_eq!(certificate.sha256.len(), 64);
  assert!(
    certificate
      .sha256
      .bytes()
      .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
  );
  let pin = orchester_api::certificate::CertificatePin::parse(&certificate.sha256).expect("a pin");
  assert_eq!(pin.check_code(), certificate.check_code);
  assert_eq!(pin.fingerprint().len(), 32 * 3 - 1);
  server.finish().await;
}

#[tokio::test]
async fn sa_admins_manage_every_project_and_owners_their_own() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let kontur = project(
    &server,
    &admin.token,
    "kontur",
    Some("https://github.com/acme/kontur.git"),
  )
  .await;
  let billing = project(&server, &admin.token, "billing", None).await;
  assert_eq!(kontur.project.key, "KTA", "a key suggested from the name");
  let members = format!("/projects/{}/members", billing.project.id);
  let owner = json!({"username": "bob", "role": "owner"});
  assert_eq!(
    server
      .request("POST", &members, Some(&admin.token), Some(owner))
      .await
      .status,
    201
  );
  let invite = json!({"kind":"project","project":billing.project.id,"role":"member","delivery":"code"});
  assert_eq!(
    server
      .request("POST", "/invites", Some(&admin.token), Some(invite))
      .await
      .status,
    201
  );
  let all: ManagedProjects = server
    .request("GET", "/admin/projects", Some(&admin.token), None)
    .await
    .json();
  let names: Vec<&str> = all
    .projects
    .iter()
    .map(|managed| managed.project.slug.as_str())
    .collect();
  assert_eq!(names, ["kontur", "billing"]);
  let billing_row = &all.projects[1];
  assert_eq!((billing_row.members, billing_row.open_invites), (2, 1));
  assert_eq!(billing_row.owners.len(), 2);
  assert_eq!(billing_row.created_by.display_name, "Alice");
  let own: ManagedProjects = server
    .request("GET", "/admin/projects", Some(&bob.token), None)
    .await
    .json();
  assert_eq!(own.projects.len(), 1);
  assert_eq!(own.projects[0].project.slug, "billing");
  let other = format!("/admin/projects/{}", kontur.project.id);
  refused(
    &server.request("GET", &other, Some(&bob.token), None).await,
    404,
    ErrorCode::NotFound,
  );
  let listed: Members = server
    .request(
      "GET",
      &format!("/projects/{}/members", kontur.project.id),
      Some(&admin.token),
      None,
    )
    .await
    .json();
  assert_eq!(listed.members[0].username, "alice");
  server.finish().await;
}

#[tokio::test]
async fn sa_the_repository_hint_is_set_and_cleared() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let kontur = project(&server, &admin.token, "kontur", None).await;
  let path = format!("/projects/{}", kontur.project.id);
  let set: Value = server
    .request(
      "PATCH",
      &path,
      Some(&admin.token),
      Some(json!({"remote_hint": "git@github.com:acme/kontur.git"})),
    )
    .await
    .json();
  assert_eq!(set["project"]["remote_hint"], json!("github.com/acme/kontur"));
  let cleared: Value = server
    .request("PATCH", &path, Some(&admin.token), Some(json!({"remote_hint": ""})))
    .await
    .json();
  assert_eq!(cleared["project"]["remote_hint"], Value::Null);
  let renamed: Value = server
    .request("PATCH", &path, Some(&admin.token), Some(json!({"name": "Kontur"})))
    .await
    .json();
  assert_eq!(renamed["project"]["slug"], json!("kontur"), "the slug never changes");
  server.finish().await;
}

#[tokio::test]
async fn sa_invites_list_their_recent_history_and_revoking_one_by_email_signs_its_revocation() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let kontur = project(&server, &admin.token, "kontur", None).await;
  let by_email = json!({"kind":"project","project":kontur.project.id,"role":"member","delivery":"cloud",
    "email":"dana@example.com"});
  let sent: Value = server
    .request("POST", "/invites", Some(&admin.token), Some(by_email))
    .await
    .json();
  let by_code = json!({"kind":"project","project":kontur.project.id,"role":"owner","delivery":"code"});
  let made: Value = server
    .request("POST", "/invites", Some(&admin.token), Some(by_code))
    .await
    .json();
  let revoked = server
    .request(
      "DELETE",
      &format!("/invites/{}", sent["invite"]["id"].as_str().expect("id")),
      Some(&admin.token),
      None,
    )
    .await;
  assert_eq!(revoked.status, 200);
  let revoked: InviteRevoked = revoked.json();
  assert_eq!(revoked.revocation.expect("signed for the cloud").split('.').count(), 3);
  let code_revoked: InviteRevoked = server
    .request(
      "DELETE",
      &format!("/invites/{}", made["invite"]["id"].as_str().expect("id")),
      Some(&admin.token),
      None,
    )
    .await
    .json();
  assert!(code_revoked.revocation.is_none(), "the cloud never knew a code");
  let open: Invites = server
    .request(
      "GET",
      &format!("/invites?project={}", kontur.project.id),
      Some(&admin.token),
      None,
    )
    .await
    .json();
  assert!(open.invites.is_empty());
  let recent: Invites = server
    .request(
      "GET",
      &format!("/invites?project={}&recent=true", kontur.project.id),
      Some(&admin.token),
      None,
    )
    .await
    .json();
  assert_eq!(recent.invites.len(), 2);
  for invite in &recent.invites {
    assert_eq!(invite.state, InviteState::Revoked);
    assert!(invite.email.is_none(), "an ended invite forgets its email");
    assert_eq!(
      invite.ended_by.as_ref().map(|user| user.display_name.as_str()),
      Some("Alice")
    );
  }
  server.clock.advance(31 * 24 * 60 * 60 * 1000);
  let later: Invites = server
    .request("GET", "/invites?recent=true", Some(&admin.token), None)
    .await
    .json();
  assert!(later.invites.is_empty(), "history is kept 30 days");
  refused(
    &server
      .request("GET", "/invites?recent=maybe", Some(&admin.token), None)
      .await,
    400,
    ErrorCode::BadRequest,
  );
  server.finish().await;
}

#[tokio::test]
async fn sa_one_person_sends_at_most_30_invites_by_email_a_day() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let invite = |index: usize| json!({"kind":"server","delivery":"cloud","email":format!("person{index}@example.com")});
  for index in 0..30 {
    let response = server
      .request("POST", "/invites", Some(&admin.token), Some(invite(index)))
      .await;
    assert_eq!(response.status, 201);
  }
  let limited = server
    .request("POST", "/invites", Some(&admin.token), Some(invite(30)))
    .await;
  refused(&limited, 429, ErrorCode::RateLimited);
  assert!(limited.headers["retry-after"].parse::<u64>().expect("seconds") > 0);
  let code = json!({"kind":"server","delivery":"code"});
  assert_eq!(
    server
      .request("POST", "/invites", Some(&admin.token), Some(code))
      .await
      .status,
    201
  );
  server.clock.advance(24 * 60 * 60 * 1000);
  assert_eq!(
    server
      .request("POST", "/invites", Some(&admin.token), Some(invite(31)))
      .await
      .status,
    201
  );
  server.finish().await;
}
