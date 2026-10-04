use orchester_api::{CreateUser, ErrorCode, UserChanges};

use crate::support::{TestServer, sign_in, token};

fn new_user(username: &str) -> CreateUser {
  CreateUser {
    username: username.into(),
    display_name: "Maria Lind".into(),
    admin: false,
  }
}

#[tokio::test]
async fn client_admin_creates_disables_enables_and_resets_users() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.admin().await;
  let client = server.client();
  let created = client.create_user(&admin, &new_user("maria")).await.expect("create");
  assert_eq!(created.user.display_name, "Maria Lind");
  assert!(!created.user.admin);
  let taken = client.create_user(&admin, &new_user("MARIA")).await.expect_err("taken");
  assert_eq!(taken.code(), Some(ErrorCode::UsernameTaken));

  let maria = token(
    &client
      .sign_in(&sign_in("maria", created.initial_password.expose()))
      .await
      .expect("first sign-in")
      .token,
  );
  let disable = UserChanges {
    disabled: Some(true),
    ..UserChanges::default()
  };
  let disabled = client
    .update_user(&admin, created.user.id, &disable)
    .await
    .expect("disable");
  assert!(disabled.disabled);
  assert_eq!(
    client.me(&maria).await.expect_err("signed out").code(),
    Some(ErrorCode::Unauthorized)
  );
  let enable = UserChanges {
    disabled: Some(false),
    ..UserChanges::default()
  };
  assert!(
    !client
      .update_user(&admin, created.user.id, &enable)
      .await
      .expect("enable")
      .disabled
  );

  let reset = client.reset_password(&admin, created.user.id).await.expect("reset");
  let again = client
    .sign_in(&sign_in("maria", reset.initial_password.expose()))
    .await
    .expect("the new first password works");
  assert!(again.password_change_required);
  let users = client.users(&admin).await.expect("users");
  assert_eq!(users.len(), 2);
  server.finish().await;
}

#[tokio::test]
async fn client_admin_cannot_disable_the_last_admin_and_members_see_no_users() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.admin().await;
  let client = server.client();
  let me = client.me(&admin).await.expect("me");
  let disable = UserChanges {
    disabled: Some(true),
    ..UserChanges::default()
  };
  let last = client
    .update_user(&admin, me.id, &disable)
    .await
    .expect_err("last admin");
  assert!(matches!(
    last,
    orchester_api::client::ClientError::Api {
      status: 409,
      code: ErrorCode::LastAdmin,
      ..
    }
  ));
  let created = client.create_user(&admin, &new_user("maria")).await.expect("create");
  let maria = token(
    &client
      .sign_in(&sign_in("maria", created.initial_password.expose()))
      .await
      .expect("sign in")
      .token,
  );
  let forbidden = client.users(&maria).await.expect_err("not an admin");
  assert!(matches!(
    forbidden.code(),
    Some(ErrorCode::Forbidden | ErrorCode::PasswordChangeRequired)
  ));
  server.finish().await;
}
