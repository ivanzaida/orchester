use orchester_api::{ApiError, ApprovalResponse, ApprovalState, Approvals, ErrorCode, Joined, SettingsResponse};
use serde_json::json;

use crate::support::{
  Response, TestServer,
  cloud::{Identity, TestCloud},
};

async fn linked_with_bob() -> (TestServer, Joined, Joined, TestCloud, String) {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let cloud = TestCloud::generate();
  let now = server.seconds();
  let linked = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(now)}})),
    )
    .await;
  let audience = linked.json::<orchester_api::CloudLinkResponse>().cloud.server;
  let invite = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"cloud","email":"bob@example.com"})),
    )
    .await
    .json::<orchester_api::InviteCreated>();
  let invite_id = invite.invite.id.to_string();
  let token = cloud.token(&identity(&audience, Some(&invite_id)), now);
  let bob: Joined = join(&server, &token, None).await.json();
  (server, admin, bob, cloud, audience)
}

fn identity<'a>(audience: &'a str, invite: Option<&'a str>) -> Identity<'a> {
  Identity {
    audience,
    subject: "b0b",
    email: "bob@example.com",
    name: "Bob",
    invite,
  }
}

async fn join(server: &TestServer, token: &str, approval: Option<&str>) -> Response {
  let mut body = json!({"identity_token": token, "device": "Bob's new laptop"});
  if let Some(approval) = approval {
    body["approval"] = json!(approval);
  }
  server.request("POST", "/auth/cloud", None, Some(body)).await
}

#[tokio::test]
async fn s1c_join_approval_is_off_by_default_and_on_makes_a_cloud_join_wait_for_another_device() {
  let (server, admin, bob, cloud, audience) = linked_with_bob().await;
  let settings: SettingsResponse = server
    .request("GET", "/server/settings", Some(&admin.token), None)
    .await
    .json();
  assert!(!settings.settings.join_approval);
  assert!(settings.check_code.is_none());
  let on = server
    .request(
      "PATCH",
      "/server/settings",
      Some(&admin.token),
      Some(json!({"join_approval": true})),
    )
    .await;
  assert!(on.json::<SettingsResponse>().settings.join_approval);
  let token = cloud.token(&identity(&audience, None), server.seconds());
  let waiting = join(&server, &token, None).await;
  assert_eq!(waiting.status, 202);
  let approval = waiting.json::<ApprovalResponse>().approval;
  assert_eq!(approval.state, ApprovalState::Waiting);
  assert_eq!(approval.user.id, bob.user.id);
  // Still waiting: asking again with the approval answers 202, and the token is not used up meanwhile.
  let id = approval.id.to_string();
  assert_eq!(join(&server, &token, Some(&id)).await.status, 202);
  let listed: Approvals = server
    .request("GET", "/auth/approvals", Some(&bob.token), None)
    .await
    .json();
  assert_eq!(listed.approvals.len(), 1);
  let admin_view: Approvals = server
    .request("GET", "/auth/approvals", Some(&admin.token), None)
    .await
    .json();
  assert_eq!(admin_view.approvals.len(), 1);
  let decide = format!("/auth/approvals/{id}");
  let approved = server
    .request("POST", &decide, Some(&bob.token), Some(json!({"approve": true})))
    .await;
  assert_eq!(
    approved.json::<ApprovalResponse>().approval.state,
    ApprovalState::Approved
  );
  let fresh = cloud.token(&identity(&audience, None), server.seconds());
  let joined = join(&server, &fresh, Some(&id)).await;
  assert_eq!(joined.status, 201);
  assert_eq!(joined.json::<Joined>().user.id, bob.user.id);
  // An approval is used once.
  let reused = join(
    &server,
    &cloud.token(&identity(&audience, None), server.seconds()),
    Some(&id),
  )
  .await;
  assert_eq!(reused.json::<ApiError>().error, ErrorCode::ApprovalRefused);
  server.finish().await;
}

#[tokio::test]
async fn s1c_a_refused_or_foreign_approval_ends_the_join() {
  let (server, admin, bob, cloud, audience) = linked_with_bob().await;
  server
    .request(
      "PATCH",
      "/server/settings",
      Some(&admin.token),
      Some(json!({"join_approval": true})),
    )
    .await;
  let carol = server.invited("carol", false).await;
  let waiting = join(
    &server,
    &cloud.token(&identity(&audience, None), server.seconds()),
    None,
  )
  .await;
  let id = waiting.json::<ApprovalResponse>().approval.id.to_string();
  let decide = format!("/auth/approvals/{id}");
  // Another account's device can neither see nor answer it.
  let others: Approvals = server
    .request("GET", "/auth/approvals", Some(&carol.token), None)
    .await
    .json();
  assert!(others.approvals.is_empty());
  let foreign = server
    .request("POST", &decide, Some(&carol.token), Some(json!({"approve": true})))
    .await;
  assert_eq!(foreign.status, 404);
  let refused = server
    .request("POST", &decide, Some(&admin.token), Some(json!({"approve": false})))
    .await;
  assert_eq!(
    refused.json::<ApprovalResponse>().approval.state,
    ApprovalState::Refused
  );
  let ended = join(
    &server,
    &cloud.token(&identity(&audience, None), server.seconds()),
    Some(&id),
  )
  .await;
  assert_eq!(ended.status, 403);
  assert_eq!(ended.json::<ApiError>().error, ErrorCode::ApprovalRefused);
  assert_eq!(
    server.request("GET", "/auth/me", Some(&bob.token), None).await.status,
    200
  );
  server.finish().await;
}
