use orchester_api::{ErrorCode, UserChanges};

use crate::support::TestServer;

#[tokio::test]
async fn client_admin_lists_disables_and_enables_users() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.admin().await;
  let (maria_id, maria) = server.person(&admin, "maria").await;
  let client = server.client();
  let disable = UserChanges {
    disabled: Some(true),
    ..UserChanges::default()
  };
  let disabled = client.update_user(&admin, maria_id, &disable).await.expect("disable");
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
      .update_user(&admin, maria_id, &enable)
      .await
      .expect("enable")
      .disabled
  );
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
  let (_, maria) = server.person(&admin, "maria").await;
  let forbidden = client.users(&maria).await.expect_err("not an admin");
  assert_eq!(forbidden.code(), Some(ErrorCode::Forbidden));
  server.finish().await;
}
