//! The person's authenticator app in the tests: it reads the secret the cloud shows when two-factor authentication is
//! turned on, and makes the codes of the cloud's clock (`orchester_api::totp`).

use orchester_api::{
  cloud::{CloudSignInChallenge, RecoveryCodes, TwoFactorSetup},
  totp,
};
use serde_json::json;

use super::{PASSWORD, Response, TestCloud, remember};

/// An account's authenticator as its app holds it, and the recovery codes shown when it was turned on.
pub struct Authenticator {
  pub secret: Vec<u8>,
  pub recovery_codes: Vec<String>,
}

impl Authenticator {
  /// The app's code `steps` steps of 30 seconds from the cloud's now.
  pub fn code(&self, cloud: &TestCloud, steps: i64) -> String {
    let now = cloud_seconds(cloud) + steps * totp::STEP_SECS;
    totp::code(&self.secret, totp::step(now))
  }
}

fn cloud_seconds(cloud: &TestCloud) -> i64 {
  use orchester_cloud::clock::Clock;
  cloud.clock.now_ms() / 1000
}

impl TestCloud {
  /// Starts turning two-factor authentication on from the device `token`; the setup, its secret remembered for the
  /// log check.
  pub async fn start_two_factor(&self, token: &str) -> TwoFactorSetup {
    let started = self
      .request(
        "POST",
        "/me/two-factor",
        Some(token),
        Some(json!({"password": PASSWORD})),
      )
      .await;
    assert_eq!(started.status, 201, "start answered {}", started.status);
    let setup: TwoFactorSetup = started.json();
    remember(setup.secret.expose());
    remember(setup.uri.expose());
    setup
  }

  /// Confirms a setup with `code`.
  pub async fn confirm_two_factor(&self, token: &str, code: &str) -> Response {
    self
      .request(
        "POST",
        "/me/two-factor/confirm",
        Some(token),
        Some(json!({"code": code})),
      )
      .await
  }

  /// Turns two-factor authentication on from the device `token`, which becomes a two-factor session.
  pub async fn enable_two_factor(&self, token: &str) -> Authenticator {
    let setup = self.start_two_factor(token).await;
    let secret = totp::decode_secret(setup.secret.expose()).expect("a base32 secret");
    let mut authenticator = Authenticator {
      secret,
      recovery_codes: Vec::new(),
    };
    let confirmed = self.confirm_two_factor(token, &authenticator.code(self, 0)).await;
    assert_eq!(confirmed.status, 200, "confirm answered {}", confirmed.status);
    let codes: RecoveryCodes = confirmed.json();
    authenticator.recovery_codes = codes
      .recovery_codes
      .iter()
      .map(|code| {
        remember(code.expose());
        code.expose().to_owned()
      })
      .collect();
    authenticator
  }

  /// The password step of a sign-in for an account with two-factor authentication: its challenge.
  pub async fn challenge(&self, email: &str, device: &str) -> String {
    let answer = self.try_sign_in(email, PASSWORD, device).await;
    assert_eq!(answer.status, 202, "the password step answered {}", answer.status);
    let challenge: CloudSignInChallenge = answer.json();
    remember(challenge.challenge.expose());
    challenge.challenge.expose().to_owned()
  }

  /// The second step of a sign-in.
  pub async fn finish_sign_in(&self, challenge: &str, code: &str) -> Response {
    let body = json!({"challenge": challenge, "code": code});
    let answer = self
      .request("POST", "/auth/sessions/two-factor", None, Some(body))
      .await;
    if answer.status == 201 {
      remember(
        answer.json::<serde_json::Value>()["token"]
          .as_str()
          .expect("a device token"),
      );
    }
    answer
  }

  /// A device token that steps up with `code`.
  pub async fn step_up(&self, token: &str, code: &str) -> Response {
    self
      .request(
        "POST",
        "/auth/sessions/current/two-factor",
        Some(token),
        Some(json!({"code": code})),
      )
      .await
  }
}
