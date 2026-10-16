//! People joining the test server as the desktop joins it: with a code, and with every code but the setup code the
//! two-factor cloud identity the server checks (`test_cloud`).

use orchester_api::{CodeJoin, Credential};
use orchester_core::secret::SecretValue;
use orchester_server::host::CloudCommand;

use super::{TestServer, test_cloud};

impl TestServer {
  /// The account `username` joined on a test computer; returns that device's token. API 3 has no passwords: the
  /// frozen `crates/desktop`'s tests use this name for what a first password was.
  pub fn create_user(&self, username: &str, display_name: &str, admin: bool) -> Credential {
    let first = self.admin.lock().0.is_none();
    let code = self.code_for(username, admin);
    // The setup code needs no cloud identity; every other code an app enters needs one with two-factor.
    let (identity_token, key_set) = if first {
      (None, None)
    } else {
      let proof = self.code_proof(username);
      (Some(proof.identity_token), proof.key_set)
    };
    let client = self.client();
    let joined = self
      .block_on(client.join_with_code(&CodeJoin {
        code,
        device: "test computer".into(),
        username: Some(username.into()),
        display_name: Some(display_name.into()),
        kind: orchester_api::DeviceKind::App,
        identity_token,
        key_set,
      }))
      .expect("join with a code");
    joined.token
  }

  /// What the app of `username`, signed in to Orchester Cloud with two-factor authentication, sends with a code: that
  /// person's identity token for codes to this server, from the tests' cloud ([`test_cloud`]), and its key set.
  pub fn code_proof(&self, username: &str) -> orchester_workspace::sharing::CodeProof {
    orchester_workspace::sharing::CodeProof {
      identity_token: Credential::new(test_cloud::code_token(&self.cloud_audience(), username)),
      key_set: Some(test_cloud::key_set(test_cloud::now())),
    }
  }

  /// The audience of this server's identity tokens, after it pinned the tests' cloud root when nothing was pinned, as
  /// its admin does with `orchester-server cloud pin`.
  pub fn cloud_audience(&self) -> String {
    let shown = self
      .block_on(self.server.host_cloud(CloudCommand::Show))
      .expect("the cloud link");
    match shown.root_fingerprint {
      Some(_) => shown.server,
      None => {
        self
          .block_on(self.server.host_cloud(CloudCommand::Pin(test_cloud::root_key())))
          .expect("pin the tests' cloud")
          .server
      }
    }
  }

  /// Another device of `username`, joined with an account code from the host: that device's token.
  pub fn device_of(&self, username: &str) -> SecretValue {
    let joined = self.join_device_of(username);
    SecretValue::new(joined.token.expose().to_owned()).expect("a token")
  }

  fn join_device_of(&self, username: &str) -> orchester_api::Joined {
    let code = self
      .block_on(self.server.host_account_code(username.into()))
      .expect("account code");
    let proof = self.code_proof(username);
    let client = self.client();
    self
      .block_on(client.join_with_code(&CodeJoin {
        code,
        device: "BUILD-02 · Windows".into(),
        username: None,
        display_name: None,
        kind: orchester_api::DeviceKind::App,
        identity_token: Some(proof.identity_token),
        key_set: proof.key_set,
      }))
      .expect("join with an account code")
  }
}
