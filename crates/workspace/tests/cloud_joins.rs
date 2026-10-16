//! Joining a team server with an Orchester Cloud identity (`docs/architecture/server.md`, row 43, "Invites"): the app
//! connects to the server the cloud names, at the cloud's address for it and trusting only the certificate the cloud
//! has for it, so an identity token for one server reaches no other.
//!
//! `cargo test -p orchester-workspace --test cloud_joins cloud_join_`

#[path = "sharing_support/cloud.rs"]
mod cloud;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::sync::Arc;

use orchester_api::{
  client::{Client, CloudClient, ServerAddress},
  cloud::CloudServer,
};
use orchester_core::{secret::SecretValue, time::Timestamp};
use orchester_store::{AppDb, StoredCloudAccount};
use orchester_workspace::{
  secrets::Vault,
  sharing::{CloudIdentity, CloudJoined, Servers, SharingError},
  workspace::Workspace,
};
use server_support::{SERVER_NAME, TestServer};
use support::MemoryVault;
use tempfile::TempDir;

/// A server with Alice as its admin, linked to a fake cloud that issues Bohdan tokens for an invite by email.
fn linked() -> (TestServer, cloud::FakeCloud, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let invite = server.email_invite("bohdan@example.com");
  let fake = cloud::FakeCloud::start(cloud::Person {
    account: BOHDAN.into(),
    email: "bohdan@example.com".into(),
    name: "Bohdan Koval".into(),
    invite: Some(invite),
  });
  let audience = server.link_cloud(&fake.root_key, &fake.key_set);
  fake.serve_for(audience.clone());
  (server, fake, audience)
}

/// Bohdan's Orchester Cloud account, whose identity the fake cloud issues.
const BOHDAN: &str = "0191c7c0-0000-7000-8000-000000000b0b";

/// The desktop's servers on a temporary application database signed in as Bohdan, and a keychain in memory.
fn desktop(dir: &TempDir) -> Servers {
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("database");
  db.set_cloud_account(Some(&StoredCloudAccount {
    account: BOHDAN.parse().expect("an account id"),
    email: "bohdan@example.com".into(),
    display_name: "Bohdan Koval".into(),
    device_name: "Laptop".into(),
    signed_in_at: Timestamp::from_unix_millis(1),
    cloud_address: "https://cloud.test".into(),
  }))
  .expect("signed in");
  Servers::new(Workspace::new(db), Vault::with(Arc::new(MemoryVault::default())))
}

#[test]
fn cloud_join_sends_nothing_to_an_address_without_the_certificate_the_cloud_named() {
  let (server, fake, audience) = linked();
  // Another server, at another address, with a certificate of its own.
  let elsewhere = TestServer::start_tls();
  let dir = TempDir::new().expect("temporary directory");
  let servers = desktop(&dir);
  let client = CloudClient::new(ServerAddress::parse(&fake.url()).expect("cloud")).expect("cloud client");
  let device = SecretValue::new("cloud device token".into()).expect("token");
  let identity = || CloudIdentity {
    cloud: &client,
    device: &device,
    account: BOHDAN.parse().expect("an account id"),
    invite: None,
  };
  let stale = CloudServer {
    id: audience.parse().expect("server id"),
    name: SERVER_NAME.into(),
    address: elsewhere.url(),
    fingerprint: Some("ab".repeat(32)),
  };
  let refused = server.block_on(servers.join_with_cloud(&stale, identity(), None));
  assert!(matches!(refused, Err(SharingError::CertificateChanged)), "{refused:?}");
  assert_eq!(fake.issued(), 0, "no identity token for the server was even asked for");
  assert!(servers.entries().expect("entries").is_empty());
  let record = CloudServer {
    address: server.url(),
    fingerprint: None,
    ..stale
  };
  let joined = server
    .block_on(servers.join_with_cloud(&record, identity(), None))
    .expect("cloud join");
  assert!(matches!(joined, CloudJoined::Joined(_)), "{joined:?}");
  assert_eq!(fake.issued(), 1);
}

#[test]
fn cloud_join_reaches_a_self_signed_server_by_the_fingerprint_the_cloud_has() {
  let (mut server, fake, audience) = linked();
  server.serve_self_signed();
  let address = ServerAddress::parse(&server.url()).expect("address");
  assert!(server.url().starts_with("https://"));
  // The fingerprint the server signed into its link statement for the cloud: its certificate's SHA-256 in hex.
  let fingerprint = server
    .block_on(Client::presented_certificate(&address))
    .expect("the server's own certificate")
    .to_hex();
  let dir = TempDir::new().expect("temporary directory");
  let servers = desktop(&dir);
  let client = CloudClient::new(ServerAddress::parse(&fake.url()).expect("cloud")).expect("cloud client");
  let device = SecretValue::new("cloud device token".into()).expect("token");
  let record = CloudServer {
    id: audience.parse().expect("server id"),
    name: SERVER_NAME.into(),
    address: server.url(),
    fingerprint: Some(fingerprint.clone()),
  };
  let identity = CloudIdentity {
    cloud: &client,
    device: &device,
    account: BOHDAN.parse().expect("an account id"),
    invite: None,
  };
  let joined = server
    .block_on(servers.join_with_cloud(&record, identity, None))
    .expect("cloud join over the pinned certificate");
  assert!(matches!(joined, CloudJoined::Joined(_)), "{joined:?}");
  let entry = servers.entries().expect("entries").remove(0);
  assert_eq!(entry.stored.certificate_pin.as_deref(), Some(fingerprint.as_str()));
  let sessions = server
    .block_on(servers.sessions(&entry))
    .expect("sessions over the pinned certificate");
  assert_eq!(sessions[0].via, orchester_api::JoinedVia::Cloud);
}
