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
  client::{CloudClient, ServerAddress},
  cloud::CloudServer,
};
use orchester_core::secret::SecretValue;
use orchester_store::AppDb;
use orchester_workspace::{
  secrets::Vault,
  sharing::{CloudIdentity, CloudJoined, Servers, SharingError},
  workspace::Workspace,
};
use server_support::{SERVER_NAME, TestServer};
use support::MemoryVault;
use tempfile::TempDir;

#[test]
fn cloud_join_sends_nothing_to_an_address_without_the_certificate_the_cloud_named() {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let invite = server.email_invite("bohdan@example.com");
  let fake = cloud::FakeCloud::start(cloud::Person {
    account: "0191c7c0-0000-7000-8000-000000000b0b".into(),
    email: "bohdan@example.com".into(),
    name: "Bohdan Koval".into(),
    invite: Some(invite),
  });
  let audience = server.link_cloud(&fake.root_key, &fake.key_set);
  fake.serve_for(audience.clone());
  // Another server, at another address, with a certificate of its own.
  let elsewhere = TestServer::start_tls();
  let dir = TempDir::new().expect("temporary directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("database"));
  let servers = Servers::new(workspace, Vault::with(Arc::new(MemoryVault::default())));
  let client = CloudClient::new(ServerAddress::parse(&fake.url()).expect("cloud")).expect("cloud client");
  let device = SecretValue::new("cloud device token".into()).expect("token");
  let identity = || CloudIdentity {
    cloud: &client,
    device: &device,
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
