//! Shared servers on the desktop (`sharing`): the reconnect backoff, how sessions name a device, the
//! new password's rule (Orchester Cloud's), and the service against a real `orchester-server` on
//! loopback: joining with a code, pairing another computer, a pinned certificate and another server's
//! refused before anything is sent, the token in the keychain, a token the server refuses, signing
//! out, the plain HTTP rule, no password sign-in, and no token or code in logs or `Debug` output.
//!
//! `cargo test -p orchester-desktop --test sharing sharing_`

#[path = "sharing_support/cloud.rs"]
mod cloud;
#[path = "sharing_support/forwarder.rs"]
mod forwarder;
#[path = "server_support/mod.rs"]
mod server_support;

use std::{
  collections::HashMap,
  io,
  sync::{Arc, OnceLock},
  time::{Duration, Instant},
};

use orchester_api::{
  Credential, ErrorCode,
  client::{ClientError, CloudClient, PlainHttp, ServerAddress},
};
use orchester_core::{
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::AppDb;
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    CloudIdentity, CloudJoined, CodeRequest, Connection, PasswordProblem, PresentedCertificate, Schedule, ServerEntry,
    Servers, SharingError, Trust, device::Device, password_problem,
  },
  workspace::Workspace,
};
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
    Self::on(Schedule::DEFAULT)
  }

  /// The desktop's servers, asked on `schedule`.
  fn on(schedule: Schedule) -> Self {
    logs();
    let dir = TempDir::new().expect("temporary directory");
    let workspace = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("database"));
    let vault = Arc::new(MemoryVault::default());
    Self {
      servers: Servers::with_schedule(workspace, Vault::with(vault.clone()), schedule),
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

/// Joins `server` from `desktop` with `code` as `username`.
fn joined(desktop: &Desktop, server: &TestServer, (username, code): (&str, &Credential)) {
  joined_at(desktop, server, &server.url(), (username, code));
}

/// [`joined`] through `url`, e.g. a forwarder in front of the server.
fn joined_at(desktop: &Desktop, server: &TestServer, url: &str, (username, code): (&str, &Credential)) {
  let address = ServerAddress::parse(url).expect("address");
  server
    .block_on(
      desktop
        .servers
        .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request(username, code)),
    )
    .expect("join with a code");
}

fn request(username: &str, code: &Credential) -> CodeRequest {
  CodeRequest {
    code: Credential::new(code.expose().into()),
    username: Some(username.into()),
    display_name: Some(username.into()),
  }
}

/// A device token as the keychain holds it.
fn secret(token: &Credential) -> SecretValue {
  SecretValue::new(token.expose().to_owned()).expect("a token")
}

#[test]
fn sharing_backoff_doubles_from_a_second_to_a_minute_then_reads_offline() {
  let waits: Vec<u64> = (1..=8)
    .map(|attempt| Schedule::DEFAULT.backoff(attempt, 0.0).as_secs())
    .collect();
  assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60]);
  assert!(Schedule::DEFAULT.backoff(3, 1.0) < Duration::from_secs(4));
  assert!(Schedule::DEFAULT.backoff(3, 1.0) >= Duration::from_millis(3200));
  let (now, since) = (Instant::now(), Timestamp::from_unix_millis(1));
  assert!(matches!(
    Schedule::DEFAULT.after_failure(6, now, since, 0.0),
    Connection::Reconnecting { attempt: 6, .. }
  ));
  assert!(
    matches!(Schedule::DEFAULT.after_failure(7, now, since, 0.0), Connection::Offline { since: at, .. } if at == since)
  );
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
fn sharing_a_code_join_keeps_the_device_token_in_the_keychain_and_the_code_works_once() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let desktop = Desktop::new();
  joined(&desktop, &server, ("alice", &code));

  let entry = desktop.entry();
  assert_eq!(entry.stored.name, SERVER_NAME);
  assert_eq!(entry.stored.certificate_pin, None);
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

  let other = Desktop::new();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let again = server.block_on(other.servers.join_with_code(
    &address,
    Trust::System(PlainHttp::NotAllowed),
    request("mallory", &code),
  ));
  assert!(matches!(again, Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::InvalidCode)));
  assert!(
    other.servers.entries().expect("entries").is_empty(),
    "a refused join adds no server"
  );
}

#[test]
fn sharing_a_pairing_code_joins_the_same_person_on_another_computer() {
  let server = TestServer::start();
  let laptop = Desktop::new();
  joined(&laptop, &server, ("alice", &server.code_for("alice", true)));
  let pairing = server
    .block_on(laptop.servers.pairing_code(&laptop.entry()))
    .expect("pairing code");
  assert_eq!(pairing.code.expose().len(), 9);

  let desk = Desktop::new();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let paired = CodeRequest {
    code: pairing.code,
    username: None,
    display_name: None,
  };
  server
    .block_on(
      desk
        .servers
        .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), paired),
    )
    .expect("join with the pairing code");
  let (one, two) = (laptop.entry(), desk.entry());
  assert_eq!(
    one.stored.account.as_ref().map(|account| account.user),
    two.stored.account.as_ref().map(|account| account.user)
  );
  let sessions = server.block_on(desk.servers.sessions(&two)).expect("sessions");
  assert_eq!(sessions.len(), 2, "one device token per computer");
  assert_ne!(laptop.tokens(), desk.tokens());
}

#[test]
fn sharing_a_pinned_certificate_is_trusted_and_another_server_s_is_refused_before_anything_is_sent() {
  let server = TestServer::start_tls();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let presented = server
    .block_on(PresentedCertificate::read(&address))
    .expect("the presented certificate");
  assert_eq!(Some(presented.check_code()), server.server.check_code());
  let trust = presented.confirm();
  let desktop = Desktop::new();
  let code = server.code_for("alice", true);
  server
    .block_on(desktop.servers.join_with_code(&address, trust, request("alice", &code)))
    .expect("join the pinned server");
  let entry = desktop.entry();
  assert!(entry.stored.certificate_pin.is_some());
  assert_eq!(
    server
      .block_on(desktop.servers.sessions(&entry))
      .expect("sessions over the pin")
      .len(),
    1
  );

  // Another server at another address presents another certificate: the confirmed pin refuses it, and its setup
  // code never left this computer.
  let impostor = TestServer::start_tls();
  let elsewhere = ServerAddress::parse(&impostor.url()).expect("address");
  let setup = impostor.code_for("eve", true);
  let refused = impostor.block_on(
    desktop
      .servers
      .join_with_code(&elsewhere, trust, request("eve", &setup)),
  );
  assert!(matches!(refused, Err(SharingError::CertificateChanged)), "{refused:?}");
  let own = impostor
    .block_on(PresentedCertificate::read(&elsewhere))
    .expect("its own certificate")
    .confirm();
  impostor
    .block_on(desktop.servers.join_with_code(&elsewhere, own, request("eve", &setup)))
    .expect("the setup code was never sent before");
}

#[test]
fn sharing_a_cloud_identity_joins_through_an_email_invite_with_the_key_set_the_app_carries() {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let invite = server.email_invite("bohdan@example.com");
  let fake = cloud::FakeCloud::start(bohdan_identity(invite));
  // The cloud names the server by its id, which linking it reveals.
  let audience = server.link_cloud(&fake.root_key, &fake.key_set);
  fake.serve_for(audience.clone());
  let desktop = Desktop::new();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let client = CloudClient::new(ServerAddress::parse(&fake.url()).expect("cloud")).expect("cloud client");
  let device = SecretValue::new("cloud device token".into()).expect("token");
  let identity = CloudIdentity {
    cloud: &client,
    device: &device,
    audience: audience.parse().expect("server id"),
  };
  let joined = server
    .block_on(
      desktop
        .servers
        .join_with_cloud(&address, Trust::System(PlainHttp::NotAllowed), identity, None),
    )
    .expect("cloud join");
  let CloudJoined::Joined(id) = joined else {
    panic!("join approval is off: {joined:?}");
  };
  let entry = desktop.entry();
  assert_eq!(entry.stored.id, id);
  let account = entry.stored.account.clone().expect("the account");
  assert_eq!(account.display_name, "Bohdan Koval");
  let sessions = server.block_on(desktop.servers.sessions(&entry)).expect("sessions");
  assert_eq!(sessions[0].via, orchester_api::JoinedVia::Cloud);
  assert!(!logs_contain(&device), "the cloud device token is never logged");
}

fn bohdan_identity(invite: String) -> cloud::Person {
  cloud::Person {
    account: "0191c7c0-0000-7000-8000-000000000b0b".into(),
    email: "bohdan@example.com".into(),
    name: "Bohdan Koval".into(),
    invite: Some(invite),
  }
}

fn logs_contain(secret: &SecretValue) -> bool {
  String::from_utf8(logs().0.lock().clone())
    .expect("UTF-8 logs")
    .contains(secret.expose())
}

#[test]
fn sharing_a_token_the_server_refuses_signs_this_computer_out() {
  let server = TestServer::start();
  let admin = secret(&server.create_user("alice", "Alice Moreau", true));
  let desktop = Desktop::new();
  joined(&desktop, &server, ("bohdan", &server.code_for("bohdan", false)));
  assert_eq!(desktop.tokens().len(), 1);

  server.reset_password(&admin, "bohdan");
  let entry = desktop.entry();
  let refused = server.block_on(desktop.servers.sessions(&entry));
  assert!(matches!(refused, Err(SharingError::SignedOut)));
  let entry = desktop.entry();
  assert_eq!(entry.connection, Some(Connection::SignedOut));
  assert!(
    entry.in_use(),
    "the account stays until the person joins again or signs out"
  );
  assert!(desktop.tokens().is_empty(), "a refused token leaves the keychain");
}

#[test]
fn sharing_sign_out_ends_the_session_and_forgets_the_token_and_the_account() {
  let server = TestServer::start();
  let desktop = Desktop::new();
  joined(&desktop, &server, ("alice", &server.code_for("alice", true)));
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
fn sharing_refuses_plain_http_to_another_computer_and_has_no_password_sign_in() {
  let desktop = Desktop::new();
  let lab = ServerAddress::parse("http://10.0.4.20:8740").expect("address");
  let runtime = tokio::runtime::Runtime::new().expect("runtime");
  let code = Credential::new("ABCD-EFGH".into());
  let refused = runtime.block_on(desktop.servers.join_with_code(
    &lab,
    Trust::System(PlainHttp::NotAllowed),
    request("bohdan", &code),
  ));
  assert!(matches!(
    refused,
    Err(SharingError::Client(ClientError::PlainHttpRefused))
  ));
  runtime
    .block_on(desktop.servers.add(&lab, "Lab".into(), PlainHttp::NotAllowed))
    .expect("add");
  let password = runtime.block_on(desktop.servers.sign_in(
    &desktop.entry(),
    "bohdan".into(),
    Credential::new("a password that is never sent".into()),
  ));
  assert!(matches!(password, Err(SharingError::NoPasswords)));
}

#[test]
fn sharing_logs_and_debug_output_show_no_token_or_code() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let desktop = Desktop::new();
  joined(&desktop, &server, ("alice", &code));
  let entry = desktop.entry();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let wrong = Credential::new("WXYZ-2345".into());
  let refused = server.block_on(desktop.servers.join_with_code(
    &address,
    Trust::System(PlainHttp::NotAllowed),
    request("alice", &wrong),
  ));
  let pairing = server.block_on(desktop.servers.pairing_code(&entry)).expect("pairing");
  let token = desktop
    .vault
    .read(&VaultKey::server_token(entry.stored.id))
    .expect("token");
  server.block_on(desktop.servers.sign_out(&entry)).expect("sign out");

  let debug = format!("{entry:?} {refused:?} {pairing:?} {:?}", desktop.servers.entries());
  let logs = String::from_utf8(logs().0.lock().clone()).expect("UTF-8 logs");
  for value in [token.expose(), code.expose(), wrong.expose(), pairing.code.expose()] {
    assert!(!debug.contains(value), "Debug output shows a secret");
    assert!(!logs.contains(value), "the log shows a secret");
  }
}

#[test]
fn sharing_the_monitor_asks_a_joined_server_after_a_restart() {
  let server = TestServer::start();
  let admin = secret(&server.create_user("alice", "Alice Moreau", true));
  let desktop = Desktop::new();
  joined(&desktop, &server, ("bohdan", &server.code_for("bohdan", false)));

  let restarted = desktop.restarted();
  let connection = || restarted.entries().expect("entries").remove(0).connection;
  assert_eq!(
    connection(),
    Some(Connection::Checking),
    "nothing is known before the server answers"
  );
  let runtime = tokio::runtime::Runtime::new().expect("the desktop's runtime");
  let monitor = runtime.spawn(orchester_workspace::sharing::monitor(restarted.clone()));
  wait_for(|| connection() == Some(Connection::Connected));

  server.reset_password(&admin, "bohdan");
  monitor.abort();
  let again = desktop.restarted();
  let connection = || again.entries().expect("entries").remove(0).connection;
  let monitor = runtime.spawn(orchester_workspace::sharing::monitor(again.clone()));
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

#[test]
fn sharing_the_monitor_retries_on_its_own_reads_offline_and_reconnects() {
  let server = TestServer::start();
  let forwarder = forwarder::Forwarder::start(server.address);
  let first = server.code_for("alice", true);
  let desktop = Desktop::on(Schedule {
    first_delay: Duration::from_millis(50),
    longest_delay: Duration::from_millis(400),
    connected_check: Duration::from_millis(100),
  });
  joined_at(&desktop, &server, &forwarder.url(), ("alice", &first));
  let runtime = tokio::runtime::Runtime::new().expect("the desktop's runtime");
  let monitor = runtime.spawn(orchester_workspace::sharing::monitor(desktop.servers.clone()));
  let connection = || desktop.entry().connection;
  wait_for(|| connection() == Some(Connection::Connected));

  forwarder.down();
  wait_for(|| matches!(connection(), Some(Connection::Reconnecting { attempt: 1, .. })));
  // Nothing asks the server but the monitor: the attempts go on by themselves.
  wait_for(|| matches!(connection(), Some(Connection::Reconnecting { attempt, .. }) if attempt >= 2));
  wait_for(|| matches!(connection(), Some(Connection::Offline { .. })));
  assert!(desktop.entry().in_use(), "offline is not signed out");

  forwarder.up();
  wait_for(|| connection() == Some(Connection::Connected));
  assert_eq!(desktop.tokens().len(), 1, "the token stayed in the keychain");
  monitor.abort();
}
