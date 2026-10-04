//! The token and passwords never show in `Debug` output or in any log line, the server's included.
//! A test binary of its own: its subscriber is the process's, so every log line of the client and
//! of the server reaches it whatever thread writes it.
//!
//! `cargo test -p orchester-api --features client --test client_redaction client_`

mod support;

use std::{io, sync::Arc};

use orchester_api::{CreateUser, Credential, PasswordChange};
use parking_lot::Mutex;
use support::{ADMIN_PASSWORD, TestServer, sign_in, token};
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
async fn client_never_shows_the_token_or_passwords_in_debug_output_or_logs() {
  let captured = Captured::default();
  let subscriber = tracing_subscriber::fmt()
    .with_max_level(tracing::Level::TRACE)
    .with_writer(captured.clone())
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("the only subscriber of this test binary");

  let server = TestServer::start(|_| {}).await;
  let admin = server.admin().await;
  let client = server.client();
  let created = client
    .create_user(
      &admin,
      &CreateUser {
        username: "bohdan".into(),
        display_name: "Bohdan Koval".into(),
        admin: false,
      },
    )
    .await
    .expect("create a user");
  let first = created.initial_password;
  let signed = client
    .sign_in(&sign_in("bohdan", first.expose()))
    .await
    .expect("sign in");
  let secret = token(&signed.token);
  let own = "bohdan's own long password";
  client
    .change_password(
      &secret,
      &PasswordChange {
        current_password: Credential::new(first.expose().into()),
        new_password: Credential::new(own.into()),
      },
    )
    .await
    .expect("choose a password");
  client.sessions(&secret).await.expect("sessions");
  client.sign_out(&secret).await.expect("sign out");
  let refused = client.me(&secret).await.expect_err("signed out");

  let debug = format!("{signed:?} {secret:?} {admin:?} {refused:?} {refused} {client:?}");
  let logs = String::from_utf8(captured.0.lock().clone()).expect("UTF-8 logs");
  assert!(
    logs.contains("request"),
    "the subscriber captured the server's request log"
  );
  for value in [signed.token.expose(), first.expose(), own, ADMIN_PASSWORD] {
    assert!(!debug.contains(value), "Debug output shows a secret");
    assert!(!logs.contains(value), "the log shows a secret");
  }
  server.finish().await;
}
