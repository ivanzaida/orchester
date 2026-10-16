use orchester_api::{CodeJoin, Credential, DeviceKind, ErrorCode, JoinedVia, client::ClientError};

use crate::support::{TestServer, token};

#[tokio::test]
async fn client_reads_what_the_server_is_and_whether_it_needs_setup() {
  let server = TestServer::start(|_| {}).await;
  let client = server.client();
  let info = client.server_info().await.expect("server info");
  assert_eq!(info.name, "Kontur team");
  assert_eq!(info.api_version, 3);
  assert!(info.needs_bootstrap);
  assert!(info.cloud.is_none());
  server.admin().await;
  assert!(!client.server_info().await.expect("server info").needs_bootstrap);
  server.finish().await;
}

#[tokio::test]
async fn client_joins_with_a_code_and_a_wrong_code_answers_invalid_code() {
  let server = TestServer::start(|_| {}).await;
  let client = server.client();
  let wrong = client
    .join_with_code(&CodeJoin {
      code: Credential::new("ZZZZ-ZZZZ".into()),
      device: "test computer".into(),
      username: Some("bohdan".into()),
      display_name: Some("Bohdan".into()),
      kind: DeviceKind::App,
      identity_token: None,
      key_set: None,
    })
    .await
    .expect_err("no such code");
  assert!(matches!(
    wrong,
    ClientError::Api {
      status: 401,
      code: ErrorCode::InvalidCode,
      ..
    }
  ));
  let code = server.server.host_setup_code().await.expect("setup").expect("no admin");
  let joined = server.join(&code, Some("bohdan")).await;
  assert!(joined.user.admin);
  assert_eq!(joined.session.via, JoinedVia::Code);
  let device = token(&joined.token);
  let sessions = client.sessions(&device).await.expect("sessions");
  assert_eq!(sessions.len(), 1);
  assert!(sessions[0].current);
  assert_eq!(sessions[0].device, "test computer · Windows");
  assert_eq!(client.me(&device).await.expect("me").username, "bohdan");
  server.finish().await;
}

#[tokio::test]
async fn client_signs_out_and_revokes_another_device() {
  let server = TestServer::start(|_| {}).await;
  let here = server.admin().await;
  let client = server.client();
  let pairing = server.post(&here, "/auth/pairings", serde_json::json!({})).await;
  let laptop = server
    .join(
      &Credential::new(pairing["code"].as_str().expect("pairing code").into()),
      None,
    )
    .await;
  let laptop_token = token(&laptop.token);
  // The setup computer, this one ([`TestServer::admin`]) and the laptop.
  assert_eq!(client.sessions(&here).await.expect("sessions").len(), 3);
  client.revoke_session(&here, laptop.session.id).await.expect("revoke");
  assert_eq!(
    client.me(&laptop_token).await.expect_err("revoked").code(),
    Some(ErrorCode::Unauthorized)
  );
  client.sign_out(&here).await.expect("sign out");
  let expired = client.me(&here).await.expect_err("the token stopped working");
  assert_eq!(expired.code(), Some(ErrorCode::Unauthorized));
  server.finish().await;
}
