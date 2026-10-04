//! Shared servers on the desktop (`sharing`): the reconnect backoff, how sessions name a device, the
//! new password's rule, and the service against a real `orchester-server` on loopback: signing in
//! with a first password, the token in the keychain, a token the server refuses, signing out, the
//! plain HTTP rule, and no token or password in logs or `Debug` output.
//!
//! `cargo test -p orchester-desktop --test sharing sharing_`

#[path = "server_support/mod.rs"]
mod server_support;

use std::{
  collections::HashMap,
  io,
  sync::{Arc, OnceLock},
  time::{Duration, Instant},
};

use orchester_api::{
  Credential,
  client::{ClientError, PlainHttp, ServerAddress},
};
use orchester_core::{
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_desktop::{
  secrets::Vault,
  sharing::{
    Connection, PasswordProblem, ServerEntry, Servers, SharingError, SignedIn, after_failure, backoff, device::Device,
    password_problem,
  },
  workspace::Workspace,
};
use orchester_store::AppDb;
use parking_lot::Mutex;
use server_support::{SERVER_NAME, TestServer};
use tempfile::TempDir;
use tracing_subscriber::fmt::MakeWriter;

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// Every log line of this test binary, from the first test that asks for it.
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

fn logs() -> &'static Captured {
  static LOGS: OnceLock<Captured> = OnceLock::new();
  LOGS.get_or_init(|| {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
      .with_max_level(tracing::Level::TRACE)
      .with_writer(captured.clone())
      .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the only subscriber of this test binary");
    captured
  })
}

/// The desktop's servers on a temporary application database and a keychain in memory.
struct Desktop {
  servers: Servers,
  vault: Arc<MemoryVault>,
  _dir: TempDir,
}

impl Desktop {
  fn new() -> Self {
    logs();
    let dir = TempDir::new().expect("temporary directory");
    let workspace = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("database"));
    let vault = Arc::new(MemoryVault::default());
    Self {
      servers: Servers::new(workspace, Vault::with(vault.clone())),
      vault,
      _dir: dir,
    }
  }

  /// The same database and keychain after the application started again.
  fn restarted(&self) -> Servers {
    let workspace = Workspace::new(AppDb::open(&self._dir.path().join("orchester.db")).expect("database"));
    Servers::new(workspace, Vault::with(self.vault.clone()))
  }

  fn entry(&self) -> ServerEntry {
    self.servers.entries().expect("entries").remove(0)
  }

  /// The tokens in the keychain.
  fn tokens(&self) -> Vec<String> {
    self.vault.0.lock().values().cloned().collect()
  }
}

/// Adds `server` and signs `username` in with the first password, choosing `own`.
fn signed_in(desktop: &Desktop, server: &TestServer, (username, first, own): (&str, &Credential, &str)) {
  let address = ServerAddress::parse(&server.url()).expect("address");
  server
    .block_on(desktop.servers.add(&address, SERVER_NAME.into(), PlainHttp::NotAllowed))
    .expect("add the server");
  let entry = desktop.entry();
  let signed = server
    .block_on(
      desktop
        .servers
        .sign_in(&entry, username.into(), Credential::new(first.expose().into())),
    )
    .expect("sign in");
  let SignedIn::PasswordChangeRequired(pending) = signed else {
    panic!("a first password asks for the person's own");
  };
  assert!(desktop.tokens().is_empty(), "the first-password session is not kept");
  server
    .block_on(desktop.servers.choose_password(
      &entry,
      &pending,
      Credential::new(first.expose().into()),
      Credential::new(own.into()),
    ))
    .expect("choose a password");
}

#[test]
fn sharing_backoff_doubles_from_a_second_to_a_minute_then_reads_offline() {
  let waits: Vec<u64> = (1..=8).map(|attempt| backoff(attempt, 0.0).as_secs()).collect();
  assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60]);
  assert!(backoff(3, 1.0) < Duration::from_secs(4));
  assert!(backoff(3, 1.0) >= Duration::from_millis(3200));
  let (now, since) = (Instant::now(), Timestamp::from_unix_millis(1));
  assert!(matches!(
    after_failure(6, now, since, 0.0),
    Connection::Reconnecting { attempt: 6, .. }
  ));
  assert!(matches!(after_failure(7, now, since, 0.0), Connection::Offline { since: at, .. } if at == since));
  assert!(Connection::SignedOut.severity() > Connection::Offline { since, next: now }.severity());
}

#[test]
fn sharing_devices_name_the_computer_and_its_platform() {
  assert_eq!(
    Device::parse("BOHDAN-PC · Windows"),
    Device::Orchester {
      computer: "BOHDAN-PC".into(),
      platform: "Windows".into()
    }
  );
  assert_eq!(Device::parse("curl"), Device::Other("curl".into()));
}

#[test]
fn sharing_new_passwords_have_12_to_256_characters_and_are_repeated() {
  assert_eq!(password_problem("", ""), Some(PasswordProblem::Empty));
  assert_eq!(
    password_problem("too short", ""),
    Some(PasswordProblem::TooShort { chars: 9 })
  );
  assert_eq!(password_problem(&"x".repeat(257), ""), Some(PasswordProblem::TooLong));
  assert_eq!(
    password_problem("a long enough phrase", ""),
    Some(PasswordProblem::NotRepeated)
  );
  assert_eq!(
    password_problem("a long enough phrase", "a long enough phrasE"),
    Some(PasswordProblem::Different)
  );
  assert_eq!(password_problem("a long enough phrase", "a long enough phrase"), None);
}

#[test]
fn sharing_keeps_the_token_in_the_keychain_once_the_person_chose_a_password() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let desktop = Desktop::new();
  signed_in(&desktop, &server, ("alice", &first, "alice's own long password"));

  let entry = desktop.entry();
  let account = entry.stored.account.clone().expect("the account is recorded");
  assert_eq!((account.username.as_str(), account.admin), ("alice", true));
  assert_eq!(entry.connection, Some(Connection::Connected));
  let key = VaultKey::server_token(entry.stored.id);
  let token = desktop.vault.read(&key).expect("the token is in the keychain");
  assert!(token.expose().starts_with("orc_"));
  assert!(server.accepts(&token));
  let sessions = server.block_on(desktop.servers.sessions(&entry)).expect("sessions");
  assert_eq!(sessions.len(), 1);
  assert!(sessions[0].current);
  assert!(matches!(Device::parse(&sessions[0].device), Device::Orchester { .. }));
}

#[test]
fn sharing_a_token_the_server_refuses_signs_this_computer_out() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let admin = server.sign_in_elsewhere("alice", first.expose(), "alice's own long password");
  let bohdan = server.create_user("bohdan", "Bohdan Koval", false);
  let desktop = Desktop::new();
  signed_in(&desktop, &server, ("bohdan", &bohdan, "bohdan's own long password"));
  assert_eq!(desktop.tokens().len(), 1);

  server.reset_password(&admin, "bohdan");
  let entry = desktop.entry();
  let refused = server.block_on(desktop.servers.sessions(&entry));
  assert!(matches!(refused, Err(SharingError::SignedOut)));
  let entry = desktop.entry();
  assert_eq!(entry.connection, Some(Connection::SignedOut));
  assert!(entry.in_use(), "the account stays until the person signs in or out");
  assert!(desktop.tokens().is_empty(), "a refused token leaves the keychain");
}

#[test]
fn sharing_sign_out_ends_the_session_and_forgets_the_token_and_the_account() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let desktop = Desktop::new();
  signed_in(&desktop, &server, ("alice", &first, "alice's own long password"));
  let token = desktop
    .vault
    .read(&VaultKey::server_token(desktop.entry().stored.id))
    .expect("token");

  server
    .block_on(desktop.servers.sign_out(&desktop.entry()))
    .expect("sign out");
  let entry = desktop.entry();
  assert_eq!((entry.stored.account.clone(), entry.connection), (None, None));
  assert!(desktop.tokens().is_empty());
  assert!(!server.accepts(&token), "the server ended the session");
}

#[test]
fn sharing_refuses_plain_http_to_another_computer_without_the_opt_in() {
  let desktop = Desktop::new();
  let lab = ServerAddress::parse("http://10.0.4.20:8740").expect("address");
  let runtime = tokio::runtime::Runtime::new().expect("runtime");
  runtime
    .block_on(desktop.servers.add(&lab, "Lab".into(), PlainHttp::NotAllowed))
    .expect("add");
  let refused = runtime.block_on(desktop.servers.sign_in(
    &desktop.entry(),
    "bohdan".into(),
    Credential::new("a password that is never sent".into()),
  ));
  assert!(matches!(
    refused,
    Err(SharingError::Client(ClientError::PlainHttpRefused))
  ));
}

#[test]
fn sharing_logs_and_debug_output_show_no_token_or_password() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let desktop = Desktop::new();
  let own = "alice's own long password";
  signed_in(&desktop, &server, ("alice", &first, own));
  let entry = desktop.entry();
  let wrong = server.block_on(desktop.servers.sign_in(
    &entry,
    "alice".into(),
    Credential::new("a wrong password here".into()),
  ));
  let token = desktop
    .vault
    .read(&VaultKey::server_token(entry.stored.id))
    .expect("token");
  server.block_on(desktop.servers.sign_out(&entry)).expect("sign out");

  let debug = format!("{entry:?} {wrong:?} {:?}", desktop.servers.entries());
  let logs = String::from_utf8(logs().0.lock().clone()).expect("UTF-8 logs");
  for secret in [token.expose(), first.expose(), own, "a wrong password here"] {
    assert!(!debug.contains(secret), "Debug output shows a secret");
    assert!(!logs.contains(secret), "the log shows a secret");
  }
}

#[test]
fn sharing_the_monitor_asks_a_signed_in_server_after_a_restart() {
  let server = TestServer::start();
  let first = server.create_user("alice", "Alice Moreau", true);
  let admin = server.sign_in_elsewhere("alice", first.expose(), "alice's own long password");
  let bohdan = server.create_user("bohdan", "Bohdan Koval", false);
  let desktop = Desktop::new();
  signed_in(&desktop, &server, ("bohdan", &bohdan, "bohdan's own long password"));

  let restarted = desktop.restarted();
  let connection = || restarted.entries().expect("entries").remove(0).connection;
  assert_eq!(
    connection(),
    Some(Connection::Checking),
    "nothing is known before the server answers"
  );
  let runtime = tokio::runtime::Runtime::new().expect("the desktop's runtime");
  let monitor = runtime.spawn(orchester_desktop::sharing::monitor(restarted.clone()));
  wait_for(|| connection() == Some(Connection::Connected));

  server.reset_password(&admin, "bohdan");
  monitor.abort();
  let again = desktop.restarted();
  let connection = || again.entries().expect("entries").remove(0).connection;
  let monitor = runtime.spawn(orchester_desktop::sharing::monitor(again.clone()));
  wait_for(|| connection() == Some(Connection::SignedOut));
  assert!(desktop.tokens().is_empty(), "the refused token left the keychain");
  monitor.abort();
}

/// Waits up to ten seconds for `done`.
fn wait_for(done: impl Fn() -> bool) {
  let start = Instant::now();
  while !done() {
    assert!(start.elapsed() < Duration::from_secs(10), "timed out");
    std::thread::sleep(Duration::from_millis(20));
  }
}
