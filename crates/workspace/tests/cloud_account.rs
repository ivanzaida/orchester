//! This computer's Orchester Cloud account and what it leads to (`docs/architecture/server.md`, "Orchester Cloud",
//! "Invites", rows 39, 43, 49–50): signing in and out, invites accepted with the certificate pin the invite carries (no
//! check code to compare) and declined, a cloud that does not deliver invites yet, joining a known server with the
//! account, a pinned server that presents another certificate, and the linked projects signing out names.
//!
//! `cargo test -p orchester-workspace --test cloud_account cloud_account_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::sync::Arc;

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use orchester_api::{
  Credential,
  client::{Client, ServerAddress},
  cloud::CloudErrorCode,
};
use orchester_core::{
  repository::RepositoryIdentity,
  secret::{SecretVault, VaultKey},
};
use orchester_store::{AppDb, JoinedWith};
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    CloudJoined, CodeRequest, Connection, PresentedCertificate, RemoteMatch, Servers, SharingError,
    cloud::{Cloud, CloudError, ServerJoin, WebPage},
  },
  workspace::Workspace,
};
use server_support::TestServer;
use support::MemoryVault;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// A desktop with a keychain in memory, using the cloud at `cloud`.
struct Desktop {
  workspace: Workspace,
  vault: Arc<MemoryVault>,
  servers: Servers,
  cloud: Cloud,
  runtime: Runtime,
  _data: TempDir,
}

impl Desktop {
  fn new(cloud: &str) -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let vault = Arc::new(MemoryVault::default());
    let servers = Servers::new(workspace.clone(), Vault::with(vault.clone()));
    let desktop = Self {
      cloud: Cloud::new(workspace.clone(), Vault::with(vault.clone()), servers.clone()),
      workspace,
      vault,
      servers,
      runtime: Runtime::new().expect("runtime"),
      _data: data,
    };
    let address = ServerAddress::parse(cloud).expect("cloud address");
    desktop
      .block_on(desktop.cloud.set_address(Some(address)))
      .expect("test cloud");
    desktop
  }

  fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  fn signed_in(cloud: &AccountCloud) -> Self {
    let desktop = Self::new(&cloud.url());
    desktop
      .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
      .expect("sign in");
    desktop
  }

  fn device_key(&self) -> VaultKey {
    VaultKey::cloud_device(self.workspace.installation_id().expect("installation"))
  }
}

/// A team server with Alice as its admin, linked to `cloud`; returns it and its id, the identity tokens' audience.
fn linked_server(cloud: &AccountCloud) -> (TestServer, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  (server, audience)
}

#[test]
fn cloud_account_signs_in_keeps_its_token_in_the_keychain_and_signs_out() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::new(&cloud.url());
  let wrong = desktop.block_on(
    desktop
      .cloud
      .sign_in(EMAIL.into(), Credential::new("wrong password".into())),
  );
  let refused = wrong.expect_err("a wrong password is refused");
  assert_eq!(refused.cloud_code(), Some(CloudErrorCode::InvalidCredentials));
  assert!(desktop.cloud.account().expect("account").is_none());

  let account = desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("sign in");
  assert_eq!(account.email, EMAIL);
  assert_eq!(desktop.cloud.account().expect("account"), Some(account));
  let stored = desktop
    .vault
    .read(&desktop.device_key())
    .expect("the token is in the keychain");
  assert!(stored.expose().starts_with("orcc_"));

  desktop.block_on(desktop.cloud.sign_out()).expect("sign out");
  assert!(cloud.signed_out(), "the cloud ended the session");
  assert!(desktop.cloud.account().expect("account").is_none());
  assert!(
    desktop.vault.read(&desktop.device_key()).is_err(),
    "the token left the keychain"
  );
}

#[test]
fn cloud_account_web_pages_and_address_follow_the_advanced_setting() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  assert_eq!(
    desktop.cloud.web_page(WebPage::SignUp).expect("page"),
    format!("{}/signup", cloud.url())
  );
  assert_eq!(
    desktop.cloud.web_page(WebPage::Forgot).expect("page"),
    format!("{}/forgot", cloud.url())
  );
  assert!(!desktop.cloud.address().expect("address").default);
  desktop
    .block_on(desktop.cloud.set_address(None))
    .expect("back to the default");
  assert!(desktop.cloud.address().expect("address").default);
  assert!(
    desktop.cloud.account().expect("account").is_none(),
    "changing the cloud signs out"
  );
}

#[test]
fn cloud_account_accepting_an_invite_connects_with_the_pin_it_carries() {
  let cloud = AccountCloud::start();
  let (mut server, audience) = linked_server(&cloud);
  let invite = server.email_invite(EMAIL);
  server.serve_self_signed();
  let desktop = Desktop::signed_in(&cloud);
  let address = ServerAddress::parse(&server.url()).expect("address");
  let pin = desktop
    .block_on(Client::presented_certificate(&address))
    .expect("the server's certificate");
  cloud.name_server(&audience, &server.url(), Some(&pin.to_hex()));
  cloud.invite(&invite, None);

  let invites = desktop.block_on(desktop.cloud.invites()).expect("invites");
  assert_eq!(invites.len(), 1);
  let accepted = desktop.block_on(desktop.cloud.accept(invites[0].id)).expect("accept");
  assert!(matches!(accepted.joined, CloudJoined::Joined(_)), "{accepted:?}");
  let entry = desktop.servers.entries().expect("entries").remove(0);
  assert_eq!(
    entry.stored.certificate_pin,
    Some(pin.to_hex()),
    "pinned from the cloud's registry"
  );
  assert_eq!(
    entry.stored.joined_with,
    Some(JoinedWith::Cloud(audience.parse().expect("id")))
  );
  assert!(desktop.block_on(desktop.cloud.invites()).expect("invites").is_empty());
}

#[test]
fn cloud_account_declining_an_invite_ends_it() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  let invite = server.email_invite(EMAIL);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, None);
  let desktop = Desktop::signed_in(&cloud);
  let id = desktop.block_on(desktop.cloud.invites()).expect("invites")[0].id;
  desktop.block_on(desktop.cloud.decline(id)).expect("decline");
  assert_eq!(cloud.declined(), vec![invite]);
  assert!(
    desktop.servers.entries().expect("entries").is_empty(),
    "nothing was joined"
  );
  let again = desktop.block_on(desktop.cloud.accept(id));
  assert!(matches!(again, Err(CloudError::InviteEnded)), "{again:?}");
}

#[test]
fn cloud_account_joins_a_server_only_with_a_place_there() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  cloud.name_server(&audience, &server.url(), None);
  let desktop = Desktop::signed_in(&cloud);
  let address = ServerAddress::parse(&server.url()).expect("address");
  let nothing = desktop
    .block_on(desktop.cloud.join_server(&address, None))
    .expect("asked");
  assert!(matches!(nothing, ServerJoin::NotInvited), "{nothing:?}");
  let by_id = Some(audience.parse().expect("id"));
  let named = desktop
    .block_on(desktop.cloud.join_server(&address, by_id))
    .expect("asked");
  assert!(
    matches!(named, ServerJoin::NotInvited),
    "the id the server names: {named:?}"
  );
  let elsewhere = ServerAddress::parse("https://orchester.elsewhere.example").expect("address");
  let unknown = desktop.block_on(desktop.cloud.join_server(&elsewhere, None));
  assert!(matches!(unknown, Err(CloudError::NoServer)), "{unknown:?}");
  let moved = desktop.block_on(desktop.cloud.join_server(&elsewhere, by_id));
  assert!(
    matches!(moved, Err(CloudError::NoServer)),
    "the registry has it elsewhere: {moved:?}"
  );
}

#[test]
fn cloud_account_a_pinned_server_with_another_certificate_reads_changed() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("bohdan", false);
  server.serve_self_signed();
  let desktop = Desktop::new("http://localhost:9");
  let address = ServerAddress::parse(&server.url()).expect("address");
  let presented = desktop
    .block_on(PresentedCertificate::read(&address))
    .expect("certificate");
  let request = CodeRequest {
    code,
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
  };
  let id = desktop
    .block_on(desktop.servers.join_with_code(&address, presented.confirm(), request))
    .expect("join with a code");
  // As if the server had made a new certificate: this computer's pin no longer matches it.
  desktop
    .workspace
    .set_server_trust(id, Some(&"ab".repeat(32)), false)
    .expect("another pin");
  let entry = || desktop.servers.entries().expect("entries").remove(0);
  let answer = desktop.block_on(desktop.servers.me(&entry()));
  assert!(matches!(answer, Err(SharingError::CertificateChanged)), "{answer:?}");
  assert!(entry().certificate_changed);
  assert_ne!(entry().connection, Some(Connection::Connected));

  let presented = desktop
    .block_on(PresentedCertificate::read(&address))
    .expect("certificate");
  desktop
    .block_on(desktop.servers.trust_again(&entry(), presented.confirm()))
    .expect("trusted again");
  desktop
    .block_on(desktop.servers.me(&entry()))
    .expect("the server answers");
  assert!(!entry().certificate_changed);
}

#[test]
fn cloud_account_linking_compares_the_origin_with_the_hint() {
  let server = TestServer::start();
  let code = server.code_for("alice", true);
  let alice = support::Person::signed_in(&server, ("alice", "Alice Moreau", &code), &["One"]);
  let hint = RepositoryIdentity::from_remote_url("https://github.com/acme/billing.git").expect("hint");
  let project = server.create_project("billing", Some("https://github.com/acme/billing.git"));
  let other = RepositoryIdentity::from_remote_url("https://github.com/acme/kontur.git").expect("origin");
  let target = Servers::target(alice.entry(), project.clone(), Some(&other));
  assert!(target.remote.warns(), "{:?}", target.remote);
  assert_eq!(
    target.remote,
    RemoteMatch::Different {
      checkout: other,
      hint: hint.clone()
    }
  );
  assert_eq!(
    Servers::target(alice.entry(), project.clone(), Some(&hint)).remote,
    RemoteMatch::Same
  );
  assert_eq!(
    Servers::target(alice.entry(), project, None).remote,
    RemoteMatch::NoOrigin
  );
  let address =
    orchester_workspace::sharing::ProjectAddress::parse(&format!("{}/billing", server.url())).expect("project address");
  let found = alice
    .block_on(alice.servers.link_target(&address, None))
    .expect("the project");
  assert_eq!(found.project.slug, "billing");
  let missing =
    orchester_workspace::sharing::ProjectAddress::parse(&format!("{}/kontr", server.url())).expect("project address");
  let none = alice.block_on(alice.servers.link_target(&missing, None));
  alice.share(&server);
  let linked = alice.servers.linked_projects(alice.entry().stored.id).expect("linked");
  assert_eq!(linked.len(), 1, "signing out names the linked project");
  assert_eq!(linked[0].name.as_str(), "Kontur");
  assert!(
    matches!(
      none,
      Err(orchester_workspace::sharing::project::link::LinkProblem::Sharing(
        SharingError::NoProject
      ))
    ),
    "{none:?}"
  );
}
