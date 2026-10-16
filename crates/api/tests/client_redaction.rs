//! Tokens and codes never show in `Debug` output or in any log line, the server's included.
//! A test binary of its own: its subscriber is the process's, so every log line of the client and
//! of the server reaches it whatever thread writes it.
//!
//! `cargo test -p orchester-api --features client --test client_redaction client_`

mod support;

use std::{io, sync::Arc};

use orchester_api::{CodeJoin, Credential, DeviceKind};
use parking_lot::Mutex;
use support::{TestServer, token};
use tracing_subscriber::fmt::MakeWriter;

/// Everything logged while it is the default subscriber, at every level.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl<'a> MakeWriter<'a> for Captured {
  type Writer = Self;

  fn make_writer(&'a self) -> Self::Writer {
    self.clone()
  }
}

#[tokio::test]
async fn client_never_shows_a_token_or_code_in_debug_output_or_logs() {
  let captured = Captured::default();
  let subscriber = tracing_subscriber::fmt()
    .with_max_level(tracing::Level::TRACE)
    .with_writer(captured.clone())
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("the only subscriber of this test binary");

  let server = TestServer::start(|_| {}).await;
  let setup = server.server.host_setup_code().await.expect("setup").expect("no admin");
  let joined = server.join(&setup, Some("alice")).await;
  let admin = token(&joined.token);
  let invite = server
    .post(
      &admin,
      "/invites",
      serde_json::json!({"kind":"server","delivery":"code"}),
    )
    .await;
  let code = invite["code"].as_str().expect("invite code").to_owned();
  let client = server.client();
  let (identity_token, key_set) = server.code_proof().await;
  let request = CodeJoin {
    code: Credential::new(code.clone()),
    device: "Bohdan's laptop".into(),
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
    kind: DeviceKind::App,
    identity_token: Some(identity_token),
    key_set: Some(key_set),
  };
  let request_debug = format!("{request:?}");
  let signed = client.join_with_code(&request).await.expect("join");
  let secret = token(&signed.token);
  client.sessions(&secret).await.expect("sessions");
  client.sign_out(&secret).await.expect("sign out");
  let refused = client.me(&secret).await.expect_err("signed out");

  let debug = format!("{signed:?} {secret:?} {admin:?} {refused:?} {refused} {client:?} {joined:?} {request_debug}");
  let logs = String::from_utf8(captured.0.lock().clone()).expect("UTF-8 logs");
  assert!(
    logs.contains("request"),
    "the subscriber captured the server's request log"
  );
  for value in [
    signed.token.expose(),
    joined.token.expose(),
    setup.expose(),
    code.as_str(),
  ] {
    assert!(!debug.contains(value), "Debug output shows a secret");
    assert!(!logs.contains(value), "the log shows a secret");
  }
  server.finish().await;
}
