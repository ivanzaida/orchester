use orchester_api::{ErrorCode, Joined, UserResponse};
use serde_json::json;

use crate::{
  cloud::{bob, bob_joined, cloud_join, email_invite, linked, refused},
  support::cloud::Identity,
};

#[tokio::test]
async fn s1c_an_admin_invite_makes_the_account_that_accepts_it_an_admin() {
  let (server, admin, cloud, audience) = linked().await;
  let known = bob_joined(&server, &admin, &cloud, &audience).await;
  assert!(!known.user.admin);
  let body = json!({"kind":"server","delivery":"cloud","email":"bob@example.com","admin":true});
  let invite = email_invite(&server, &admin.token, body).await;
  let invite_id = invite.invite.id.to_string();
  let joined = cloud_join(
    &server,
    &cloud.token(&bob(&audience, Some(&invite_id)), server.seconds()),
    None,
  )
  .await;
  assert_eq!(joined.status, 201);
  let joined: Joined = joined.json();
  assert_eq!(joined.user.id, known.user.id);
  assert!(joined.user.admin);
  let me: UserResponse = server.request("GET", "/auth/me", Some(&known.token), None).await.json();
  assert!(me.user.admin);
  server.finish().await;
}

#[tokio::test]
async fn s1c_an_account_invite_refuses_the_identity_of_another_account_and_stays_open() {
  let (server, admin, cloud, audience) = linked().await;
  let known = bob_joined(&server, &admin, &cloud, &audience).await;
  let carol = server.invited("carol", false).await;
  // Meant for Carol's account, sent to an email whose identity is already Bob's.
  let body = json!({"kind":"account","user":carol.user.id,"delivery":"cloud","email":"bob@example.com"});
  let invite = email_invite(&server, &admin.token, body).await;
  let invite_id = invite.invite.id.to_string();
  refused(
    &cloud_join(
      &server,
      &cloud.token(&bob(&audience, Some(&invite_id)), server.seconds()),
      None,
    )
    .await,
    403,
    ErrorCode::NotInvited,
  );
  let (accepted, carol_linked): (Option<i64>, bool) = server
    .database()
    .query_row(
      "SELECT (SELECT accepted_at FROM invites WHERE id=?1), (SELECT cloud_subject IS NOT NULL FROM users WHERE id=?2)",
      [invite_id, carol.user.id.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("invite and account");
  assert_eq!((accepted, carol_linked), (None, false));
  let me: UserResponse = server.request("GET", "/auth/me", Some(&known.token), None).await.json();
  assert_eq!(me.user.id, known.user.id);
  server.finish().await;
}

#[tokio::test]
async fn s1c_a_blank_or_overlong_cloud_name_is_a_counted_refusal() {
  let (server, admin, cloud, audience) = linked().await;
  bob_joined(&server, &admin, &cloud, &audience).await;
  let long = "B".repeat(257);
  for name in [" ", long.as_str()] {
    let identity = Identity {
      name,
      ..bob(&audience, None)
    };
    refused(
      &cloud_join(&server, &cloud.token(&identity, server.seconds()), None).await,
      401,
      ErrorCode::InvalidIdentity,
    );
  }
  let identity = Identity {
    name: &long[..256],
    ..bob(&audience, None)
  };
  assert_eq!(
    cloud_join(&server, &cloud.token(&identity, server.seconds()), None)
      .await
      .status,
    201
  );
  let counted: i64 = server
    .database()
    .query_row("SELECT SUM(count) FROM failed_joins WHERE kind='cloud'", [], |row| {
      row.get(0)
    })
    .expect("failed cloud joins");
  assert_eq!(counted, 2);
  server.finish().await;
}

#[tokio::test]
async fn s1c_one_identity_token_joins_once_however_many_use_it_at_once() {
  let (server, admin, cloud, audience) = linked().await;
  bob_joined(&server, &admin, &cloud, &audience).await;
  let token = cloud.token(&bob(&audience, None), server.seconds());
  let (a, b, c, d) = tokio::join!(
    cloud_join(&server, &token, None),
    cloud_join(&server, &token, None),
    cloud_join(&server, &token, None),
    cloud_join(&server, &token, None),
  );
  let statuses = [a.status, b.status, c.status, d.status];
  assert_eq!(
    statuses.iter().filter(|status| **status == 201).count(),
    1,
    "{statuses:?}"
  );
  assert_eq!(
    statuses.iter().filter(|status| **status == 401).count(),
    3,
    "{statuses:?}"
  );
  server.finish().await;
}
