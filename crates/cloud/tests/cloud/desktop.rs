//! The desktop's calls to Orchester Cloud (N1: `orchester-api`'s `CloudClient`, which `orchester-workspace`'s
//! `sharing::cloud` and so Settings › Orchester use) against this cloud on loopback: an account confirmed through the
//! API signs a desktop in, a wrong password is refused, a new account has no invites and no servers, and signing out
//! ends the token. An account with two-factor authentication is told apart (`ClientError::TwoFactorRequired`).
//! Accepting an invite with a token that names it is in `accept.rs`; the whole flow through the app is in
//! `crates/desktop-app/tests/orchester_cloud`. The desktop's own handling (the keychain, the account row) is tested in
//! `crates/workspace/tests/cloud_account.rs`; neither the workspace nor the app can be built into this binary without
//! breaking clippy's runs (`tools/check.ps1`).

use orchester_api::{
  Credential,
  client::{ClientError, CloudClient, ServerAddress},
  cloud::{CloudErrorCode, CloudPlatform, CloudSignIn, DeviceInfo},
};
use orchester_core::secret::SecretValue;

use crate::support::{PASSWORD, TestCloud, remember};

fn sign_in(email: &str, password: &str) -> CloudSignIn {
  CloudSignIn {
    email: email.to_owned(),
    password: Credential::new(password.to_owned()),
    device: DeviceInfo {
      name: "IVAN-PC".into(),
      platform: CloudPlatform::Windows,
    },
  }
}

pub async fn desktop_signs_in_and_out_with_a_confirmed_account() {
  let cloud = TestCloud::start().await;
  let email = "desktop@example.test";
  cloud.register(email).await;
  let address = ServerAddress::parse(&format!("http://localhost:{}", cloud.address().port())).expect("address");
  let client = CloudClient::new(address).expect("client");

  remember("not the password");
  let wrong = client.sign_in(&sign_in(email, "not the password")).await;
  assert!(
    matches!(
      wrong,
      Err(ClientError::Cloud {
        code: CloudErrorCode::InvalidCredentials,
        ..
      })
    ),
    "{wrong:?}"
  );

  let signed = client.sign_in(&sign_in(email, PASSWORD)).await.expect("sign in");
  remember(signed.token.expose());
  assert_eq!(signed.account.email, email);
  let token = SecretValue::new(signed.token.expose().to_owned()).expect("a token");
  let me = client.me(&token).await.expect("the account");
  assert_eq!(me.account.id, signed.account.id);

  assert!(client.invites(&token).await.expect("invites").is_empty());
  assert!(client.servers(&token).await.expect("servers").is_empty());

  cloud.enable_two_factor(signed.token.expose()).await;
  let challenged = client.sign_in(&sign_in(email, PASSWORD)).await;
  assert!(
    matches!(challenged, Err(ClientError::TwoFactorRequired)),
    "a challenge is said as such, not as another server: {challenged:?}"
  );

  client.sign_out(&token).await.expect("sign out");
  let after = client.me(&token).await;
  assert!(
    matches!(
      after,
      Err(ClientError::Cloud {
        code: CloudErrorCode::Unauthorized,
        ..
      })
    ),
    "the cloud ended the token: {after:?}"
  );
  cloud.finish().await;
}
