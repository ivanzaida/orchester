use orchester_api::{
  ApiError, Credential, ErrorCode, InviteCreated, InviteKind, Invites, MemberRole, Members, ProjectCreated,
};
use serde_json::{Value, json};

use crate::support::{Response, TestServer};

async fn project(server: &TestServer, token: &Credential, slug: &str) -> ProjectCreated {
  let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": slug, "slug": slug,
    "key": "KON", "tasks": []});
  let response = server.request("POST", "/projects", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

async fn invite(server: &TestServer, token: &Credential, body: Value) -> Response {
  server.request("POST", "/invites", Some(token), Some(body)).await
}

#[tokio::test]
async fn s1c_a_project_invite_code_makes_an_account_with_that_project_only() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let a = project(&server, &admin.token, "project-a").await;
  let b = project(&server, &admin.token, "project-b").await;
  let created: InviteCreated = invite(
    &server,
    &admin.token,
    json!({"kind":"project","project":a.project.id,"role":"owner","delivery":"code"}),
  )
  .await
  .json();
  assert_eq!(created.invite.kind, InviteKind::Project);
  let bob = server
    .join(created.code.as_ref().expect("code"), Some("bob"), "laptop")
    .await;
  let joined = bob.joined.expect("membership");
  assert_eq!((joined.project, joined.role), (a.project.id, MemberRole::Owner));
  let members: Members = server
    .request(
      "GET",
      &format!("/projects/{}/members", a.project.id),
      Some(&bob.token),
      None,
    )
    .await
    .json();
  assert_eq!(members.members.len(), 2);
  let other = format!("/projects/{}", b.project.id);
  assert_eq!(server.request("GET", &other, Some(&bob.token), None).await.status, 404);
  // As an owner of project-a, Bob invites to it, and only to it.
  let carol = invite(
    &server,
    &bob.token,
    json!({"kind":"project","project":a.project.id,"role":"member","delivery":"code"}),
  )
  .await;
  assert_eq!(carol.status, 201);
  let elsewhere = invite(
    &server,
    &bob.token,
    json!({"kind":"project","project":b.project.id,"role":"member","delivery":"code"}),
  )
  .await;
  assert_eq!(elsewhere.status, 404);
  let server_invite = invite(&server, &bob.token, json!({"kind":"server","delivery":"code"})).await;
  assert_eq!(server_invite.status, 403);
  let carol = server
    .join(
      carol.json::<InviteCreated>().code.as_ref().expect("code"),
      Some("carol"),
      "laptop",
    )
    .await;
  let as_member = invite(
    &server,
    &carol.token,
    json!({"kind":"project","project":a.project.id,"role":"member","delivery":"code"}),
  )
  .await;
  assert_eq!(as_member.status, 403);
  server.finish().await;
}

#[tokio::test]
async fn s1c_invites_are_listed_without_codes_and_a_revoked_invite_stops_working() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  // The admin sends invites by email, which needs a two-factor sign-in (accounts/two_factor.rs).
  server.two_factor(&admin.token);
  let a = project(&server, &admin.token, "project-a").await;
  let by_code: InviteCreated = invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"code","admin":true}),
  )
  .await
  .json();
  let by_email: InviteCreated = invite(
    &server,
    &admin.token,
    json!({"kind":"project","project":a.project.id,"role":"member","delivery":"cloud","email":"Bob@Example.com"}),
  )
  .await
  .json();
  assert_eq!(by_email.invite.email.as_deref(), Some("bob@example.com"));
  let listed = server.request("GET", "/invites", Some(&admin.token), None).await;
  let invites: Invites = listed.json();
  assert_eq!(invites.invites.len(), 2);
  assert!(!listed.contains(by_code.code.as_ref().expect("code").expose()));
  let revoke = format!("/invites/{}", by_code.invite.id);
  assert_eq!(
    server.request("DELETE", &revoke, Some(&admin.token), None).await.status,
    200
  );
  assert_eq!(
    server.request("DELETE", &revoke, Some(&admin.token), None).await.status,
    404
  );
  let refused = server
    .redeem(by_code.code.as_ref().expect("code"), Some("bob"), "laptop")
    .await;
  assert_eq!(refused.json::<ApiError>().error, ErrorCode::InvalidCode);
  let revoke_email = format!("/invites/{}", by_email.invite.id);
  assert_eq!(
    server
      .request("DELETE", &revoke_email, Some(&admin.token), None)
      .await
      .status,
    200
  );
  let email: Option<String> = server
    .database()
    .query_row(
      "SELECT email FROM invites WHERE id=?1",
      [by_email.invite.id.to_string()],
      |row| row.get(0),
    )
    .expect("ended invite");
  assert!(email.is_none(), "an ended invite forgets its email");
  let remaining: Invites = server.request("GET", "/invites", Some(&admin.token), None).await.json();
  assert!(remaining.invites.is_empty());
  server.finish().await;
}

#[tokio::test]
async fn s1c_invite_shapes_are_checked() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  for body in [
    json!({"kind":"server","delivery":"cloud"}),
    json!({"kind":"server","delivery":"code","email":"bob@example.com"}),
    json!({"kind":"server","delivery":"cloud","email":"not an email"}),
    json!({"kind":"project","delivery":"code"}),
    json!({"kind":"account","delivery":"code"}),
  ] {
    let response = invite(&server, &admin.token, body.clone()).await;
    assert_eq!(response.status, 422, "{body}");
  }
  let missing = invite(
    &server,
    &admin.token,
    json!({"kind":"account","delivery":"code","user":orchester_core::id::UserId::generate()}),
  )
  .await;
  assert_eq!(missing.status, 404);
  let unknown_field = invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"code","extra":1}),
  )
  .await;
  assert_eq!(unknown_field.status, 400);
  server.finish().await;
}

#[tokio::test]
async fn s1c_admins_make_account_invites_only_for_accounts_that_are_not_admins() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let olena = server.invited("olena", true).await;
  let bob = server.invited("bob", false).await;
  for user in [olena.user.id, admin.user.id] {
    for body in [
      json!({"kind":"account","delivery":"code","user":user}),
      json!({"kind":"account","delivery":"cloud","user":user,"email":"someone@example.com"}),
    ] {
      let refused = invite(&server, &admin.token, body.clone()).await;
      assert_eq!(refused.status, 403, "{body}");
      assert_eq!(refused.json::<ApiError>().error, ErrorCode::Forbidden);
    }
  }
  let created = invite(
    &server,
    &olena.token,
    json!({"kind":"account","delivery":"code","user":bob.user.id}),
  )
  .await;
  assert_eq!(created.status, 201);
  server.finish().await;
}

async fn set_admin(server: &TestServer, admin: &Credential, user: orchester_core::id::UserId, changes: Value) {
  let path = format!("/admin/users/{user}");
  assert_eq!(
    server.request("PATCH", &path, Some(admin), Some(changes)).await.status,
    200
  );
}

#[tokio::test]
async fn s1c_an_account_code_is_refused_once_its_account_became_an_admin() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let created: InviteCreated = invite(
    &server,
    &admin.token,
    json!({"kind":"account","delivery":"code","user":bob.user.id}),
  )
  .await
  .json();
  let code = created.code.as_ref().expect("code");
  set_admin(&server, &admin.token, bob.user.id, json!({"admin": true})).await;
  let refused = server.redeem(code, None, "new laptop").await;
  assert_eq!(refused.status, 403);
  assert_eq!(refused.json::<ApiError>().error, ErrorCode::Forbidden);
  // The code stays open: once Bob is no admin again it works.
  set_admin(&server, &admin.token, bob.user.id, json!({"admin": false})).await;
  assert_eq!(server.redeem(code, None, "new laptop").await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn s1c_an_admin_invite_is_refused_once_its_inviter_is_no_enabled_admin() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let olena = server.invited("olena", true).await;
  let mut codes = Vec::new();
  for _ in 0..2 {
    let created: InviteCreated = invite(
      &server,
      &olena.token,
      json!({"kind":"server","delivery":"code","admin":true}),
    )
    .await
    .json();
    codes.push(created.code.expect("code"));
  }
  set_admin(&server, &admin.token, olena.user.id, json!({"admin": false})).await;
  let refused = server.redeem(&codes[0], Some("mallory"), "laptop").await;
  assert_eq!(refused.status, 403);
  assert_eq!(refused.json::<ApiError>().error, ErrorCode::Forbidden);
  set_admin(
    &server,
    &admin.token,
    olena.user.id,
    json!({"admin": true, "disabled": true}),
  )
  .await;
  assert_eq!(server.redeem(&codes[1], Some("mallory"), "laptop").await.status, 403);
  let made: i64 = server
    .database()
    .query_row("SELECT COUNT(*) FROM users WHERE username='mallory'", [], |row| {
      row.get(0)
    })
    .expect("accounts");
  assert_eq!(made, 0);
  set_admin(&server, &admin.token, olena.user.id, json!({"disabled": false})).await;
  assert_eq!(server.redeem(&codes[1], Some("mallory"), "laptop").await.status, 201);
  server.finish().await;
}
