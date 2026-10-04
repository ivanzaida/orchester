use orchester_api::{Credential, ErrorCode, PasswordChange, client::ClientError};

use crate::support::{ADMIN_PASSWORD, TestServer, sign_in, token};

#[tokio::test]
async fn client_reads_what_the_server_is_and_whether_it_has_an_admin() {
  let server = TestServer::start(|_| {}).await;
  let client = server.client();
  let info = client.server_info().await.expect("server info");
  assert_eq!(info.name, "Kontur team");
  assert_eq!(info.api_version, 1);
  assert!(info.needs_bootstrap);
  server.admin().await;
  assert!(!client.server_info().await.expect("server info").needs_bootstrap);
  server.finish().await;
}

#[tokio::test]
async fn client_signs_in_with_a_first_password_and_must_choose_its_own() {
  let server = TestServer::start(|_| {}).await;
  let first = server.create_user("bohdan", true).await;
  let client = server.client();
  let signed = client
    .sign_in(&sign_in("bohdan", first.expose()))
    .await
    .expect("sign in");
  assert!(signed.password_change_required);
  assert_eq!(signed.user.username, "bohdan");
  let restricted = token(&signed.token);
  let refused = client
    .sessions(&restricted)
    .await
    .expect_err("only the password change works");
  assert_eq!(refused.code(), Some(ErrorCode::PasswordChangeRequired));

  let short = PasswordChange {
    current_password: Credential::new(first.expose().into()),
    new_password: Credential::new("too short".into()),
  };
  let invalid = client
    .change_password(&restricted, &short)
    .await
    .expect_err("12 characters at least");
  assert_eq!(invalid.code(), Some(ErrorCode::Invalid));
  let own = PasswordChange {
    current_password: Credential::new(first.expose().into()),
    new_password: Credential::new("bohdan's own long password".into()),
  };
  client
    .change_password(&restricted, &own)
    .await
    .expect("choose a password");
  let sessions = client.sessions(&restricted).await.expect("the session works now");
  assert_eq!(sessions.len(), 1);
  assert!(sessions[0].current);
  assert_eq!(sessions[0].device, "test computer · Windows");
  assert_eq!(client.me(&restricted).await.expect("me").username, "bohdan");
  server.finish().await;
}

#[tokio::test]
async fn client_reports_wrong_credentials_and_signed_out_tokens() {
  let server = TestServer::start(|_| {}).await;
  server.admin().await;
  let client = server.client();
  let wrong = client
    .sign_in(&sign_in("alice", "not the password at all"))
    .await
    .expect_err("wrong password");
  assert!(matches!(
    wrong,
    ClientError::Api {
      status: 401,
      code: ErrorCode::InvalidCredentials,
      ..
    }
  ));
  let unknown = client
    .sign_in(&sign_in("nobody", "not the password at all"))
    .await
    .expect_err("unknown user");
  assert_eq!(unknown.code(), Some(ErrorCode::InvalidCredentials));

  let signed = client
    .sign_in(&sign_in("alice", ADMIN_PASSWORD))
    .await
    .expect("sign in");
  let token = token(&signed.token);
  client.me(&token).await.expect("signed in");
  client.sign_out(&token).await.expect("sign out");
  let expired = client.me(&token).await.expect_err("the token stopped working");
  assert_eq!(expired.code(), Some(ErrorCode::Unauthorized));
  server.finish().await;
}

#[tokio::test]
async fn client_revokes_another_session_and_a_password_change_signs_the_others_out() {
  let server = TestServer::start(|_| {}).await;
  let here = server.admin().await;
  let client = server.client();
  let laptop = token(
    &client
      .sign_in(&sign_in("alice", ADMIN_PASSWORD))
      .await
      .expect("laptop")
      .token,
  );
  let build = token(
    &client
      .sign_in(&sign_in("alice", ADMIN_PASSWORD))
      .await
      .expect("build")
      .token,
  );
  let sessions = client.sessions(&here).await.expect("sessions");
  assert_eq!(sessions.len(), 3);
  let laptop_session = client
    .sessions(&laptop)
    .await
    .expect("sessions")
    .into_iter()
    .find(|session| session.current)
    .expect("the laptop's session")
    .id;
  client.revoke_session(&here, laptop_session).await.expect("revoke");
  assert_eq!(
    client.me(&laptop).await.expect_err("revoked").code(),
    Some(ErrorCode::Unauthorized)
  );

  let change = PasswordChange {
    current_password: Credential::new(ADMIN_PASSWORD.into()),
    new_password: Credential::new("another secure admin password".into()),
  };
  client.change_password(&here, &change).await.expect("change password");
  assert_eq!(
    client.me(&build).await.expect_err("signed out").code(),
    Some(ErrorCode::Unauthorized)
  );
  client.me(&here).await.expect("this computer stays signed in");
  server.finish().await;
}
