//! An invite by email is not made twice (`docs/architecture/server.md`, "Invites"): not while its address has an open
//! one for the same project (or the server), and not to someone who is a member already. The check runs in the
//! transaction that makes the invite, so two quick submits make one invite. Codes are tied to no address.

use orchester_api::{Credential, ErrorCode, InviteCreated, ProjectCreated, SharingError, SharingErrorDetails};
use serde_json::{Value, json};

use crate::{
  cloud::{bob, cloud_join, email_invite, linked, refused},
  support::{Response, TestServer},
};

const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;

async fn project(server: &TestServer, token: &Credential, slug: &str) -> ProjectCreated {
  let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": slug, "slug": slug,
    "key": "KON", "tasks": []});
  let response = server.request("POST", "/projects", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

async fn invite(server: &TestServer, token: &Credential, body: &Value) -> Response {
  server
    .request("POST", "/invites", Some(token), Some(body.clone()))
    .await
}

fn to_project(project: &ProjectCreated, email: &str) -> Value {
  json!({"kind":"project","project":project.project.id,"role":"member","delivery":"cloud","email":email})
}

#[tokio::test]
async fn repeated_an_open_email_invite_is_not_sent_again_until_it_ends() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let kontur = project(&server, &admin.token, "kontur").await;
  let billing = project(&server, &admin.token, "billing").await;
  let first = email_invite(&server, &admin.token, to_project(&kontur, "slyapa@tuxick.dev")).await;
  server.clock.advance(60_000);
  let again = invite(&server, &admin.token, &to_project(&kontur, "Slyapa@Tuxick.dev")).await;
  refused(&again, 409, ErrorCode::InviteOpen);
  assert_eq!(
    again.json::<SharingError>().details,
    SharingErrorDetails::Invite {
      invite: first.invite.id,
      sent_at: first.invite.created_at
    },
    "the refusal names the open invite and when it was sent"
  );
  for other in [
    to_project(&billing, "slyapa@tuxick.dev"),
    json!({"kind":"server","delivery":"cloud","email":"slyapa@tuxick.dev"}),
    json!({"kind":"project","project":kontur.project.id,"role":"member","delivery":"code"}),
  ] {
    assert_eq!(invite(&server, &admin.token, &other).await.status, 201, "{other}");
  }
  let revoke = format!("/invites/{}", first.invite.id);
  assert_eq!(
    server.request("DELETE", &revoke, Some(&admin.token), None).await.status,
    200
  );
  let after_revoke = email_invite(&server, &admin.token, to_project(&kontur, "slyapa@tuxick.dev")).await;
  server.clock.advance(WEEK_MS);
  let after_expiry = invite(&server, &admin.token, &to_project(&kontur, "slyapa@tuxick.dev")).await;
  assert_eq!(after_expiry.status, 201, "an expired invite is not open");
  assert_ne!(after_revoke.invite.id, after_expiry.json::<InviteCreated>().invite.id);
  server.finish().await;
}

#[tokio::test]
async fn repeated_two_quick_submits_make_one_invite() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let kontur = project(&server, &admin.token, "kontur").await;
  let body = to_project(&kontur, "slyapa@tuxick.dev");
  let (one, two) = tokio::join!(
    invite(&server, &admin.token, &body),
    invite(&server, &admin.token, &body)
  );
  let mut statuses = [one.status, two.status];
  statuses.sort_unstable();
  assert_eq!(statuses, [201, 409]);
  let made: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM invites WHERE email='slyapa@tuxick.dev'",
      [],
      |row| row.get(0),
    )
    .expect("invites");
  assert_eq!(made, 1);
  server.finish().await;
}

#[tokio::test]
async fn repeated_a_member_is_not_invited_again() {
  let (server, admin, cloud, audience) = linked().await;
  let kontur = project(&server, &admin.token, "kontur").await;
  let billing = project(&server, &admin.token, "billing").await;
  let invited = email_invite(&server, &admin.token, to_project(&kontur, "bob@example.com")).await;
  let invite_id = invited.invite.id.to_string();
  let token = cloud.token(&bob(&audience, Some(&invite_id)), server.seconds());
  assert_eq!(cloud_join(&server, &token, None).await.status, 201);
  // Bob's identity says `Bob@Example.com`: the address is compared as the server keeps it, lowercased.
  refused(
    &invite(&server, &admin.token, &to_project(&kontur, "bob@example.com")).await,
    409,
    ErrorCode::AlreadyMember,
  );
  refused(
    &invite(
      &server,
      &admin.token,
      &json!({"kind":"server","delivery":"cloud","email":"BOB@example.com"}),
    )
    .await,
    409,
    ErrorCode::AlreadyMember,
  );
  assert_eq!(
    invite(&server, &admin.token, &to_project(&billing, "bob@example.com"))
      .await
      .status,
    201,
    "a project Bob is not a member of"
  );
  let as_admin = json!({"kind":"server","delivery":"cloud","email":"bob@example.com","admin":true});
  assert_eq!(
    invite(&server, &admin.token, &as_admin).await.status,
    201,
    "an invite as an admin gives Bob more"
  );
  server.finish().await;
}
