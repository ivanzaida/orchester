use orchester_api::{ApiError, ErrorCode, UserResponse};
use serde_json::json;

use crate::support::TestServer;

fn audit_count(server: &TestServer) -> i64 {
  server
    .database()
    .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
    .expect("audit count")
}

#[tokio::test]
async fn s1a_request_types_reject_unknown_fields_and_user_noops_are_not_audited() {
  let server = TestServer::start(|_| {}).await;
  let signed = server.bootstrap().await;
  let user_path = format!("/admin/users/{}", signed.user.id);
  let before = audit_count(&server);
  for (path, method, body) in [
    (
      "/auth/sessions",
      "POST",
      json!({"username":"alice", "password":"a secure admin password", "device":"test", "extra":signed.token.expose()}),
    ),
    (
      "/auth/password",
      "POST",
      json!({"current_password":"a secure admin password", "new_password":"new admin password", "extra":signed.token.expose()}),
    ),
    (
      "/admin/users",
      "POST",
      json!({"username":"bob", "display_name":"Bob", "admin":false, "extra":signed.token.expose()}),
    ),
    (
      user_path.as_str(),
      "PATCH",
      json!({"username":"renamed", "display_name":"Modified", "extra":signed.token.expose()}),
    ),
  ] {
    let response = server.request(method, path, Some(&signed.token), Some(body)).await;
    assert_eq!(response.status, 400);
    assert_eq!(response.json::<ApiError>().error, ErrorCode::BadRequest);
    assert!(!response.contains(signed.token.expose()));
  }
  assert_eq!(audit_count(&server), before);
  for body in [
    json!({}),
    json!({"display_name":"Alice", "admin":true, "disabled":false}),
    json!({"display_name":null}),
  ] {
    let response = server
      .request("PATCH", &user_path, Some(&signed.token), Some(body))
      .await;
    assert_eq!(response.status, 200);
    assert_eq!(response.json::<UserResponse>().user.display_name, "Alice");
  }
  assert_eq!(audit_count(&server), before);
  assert_eq!(
    server
      .request(
        "PATCH",
        &user_path,
        Some(&signed.token),
        Some(json!({"display_name":"Updated Alice"}))
      )
      .await
      .status,
    200
  );
  assert_eq!(audit_count(&server), before + 1);
  server.finish().await;
}
