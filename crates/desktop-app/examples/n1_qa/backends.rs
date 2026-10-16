//! What N1's QA talks to (shared by `n1_qa` and `n1_backends`): a real `orchester-server` on loopback with Alice as its
//! admin and the projects Kontur, Billing and Mobile app, linked to a stand-in for Orchester Cloud's account API that
//! names it. Fixture data for QA, never shipped.

#![allow(dead_code, reason = "shared by n1_qa and n1_backends, each of which uses a part")]

use std::path::Path;

use orchester_api::{
  Credential, SharedProject,
  certificate::CertificatePin,
  client::{Client, ServerAddress},
};
use orchester_store::AppDb;
use orchester_workspace::{
  secrets::Vault,
  sharing::{Servers, cloud::Cloud},
  workspace::Workspace,
};
use tokio::runtime::Runtime;

use super::{
  account_cloud::{AccountCloud, EMAIL, PASSWORD},
  server_support::TestServer,
  vault::QaVault,
};

pub struct Backends {
  pub cloud: AccountCloud,
  pub server: TestServer,
  pub kontur: SharedProject,
  pub billing: SharedProject,
  /// The server's id at the cloud.
  audience: String,
  /// The certificate the server presents; `None` on plain HTTP to this computer.
  pub pin: Option<CertificatePin>,
}

impl Backends {
  /// The cloud and the server; `plain` keeps the server on plain HTTP to this computer, which needs no check code.
  pub fn start(plain: bool) -> Self {
    let cloud = AccountCloud::start();
    let mut server = TestServer::start();
    server.create_user("alice", "Alice Moreau", true);
    let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
    let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some("https://github.com/acme/kontur.git"));
    let billing = server.create_named_project(
      ("Billing", "BIL"),
      "billing",
      Some("https://github.com/acme/billing.git"),
    );
    server.create_named_project(("Mobile app", "MOB"), "mobile-app", None);
    let pin = (!plain).then(|| {
      server.serve_self_signed();
      let address = ServerAddress::parse(&server.url()).expect("address");
      server
        .block_on(Client::presented_certificate(&address))
        .expect("the server's certificate")
    });
    let backends = Self {
      cloud,
      server,
      kontur,
      billing,
      audience,
      pin,
    };
    backends.named_at(None);
    backends
  }

  /// Has the cloud name the server at `address`, or at its own address with its certificate.
  pub fn named_at(&self, address: Option<&str>) {
    match address {
      Some(address) => self.cloud.name_server(&self.audience, address, None),
      None => self.cloud.name_server(
        &self.audience,
        &self.server.url(),
        self.pin.map(|pin| pin.to_hex()).as_deref(),
      ),
    }
  }

  /// An invite to Billing for the cloud's account, delivered through the cloud; its id.
  pub fn invite_to_billing(&self) -> String {
    let invite = self.server.project_email_invite(EMAIL, &self.billing);
    let address = format!("{}/billing", self.server.url());
    self.cloud.invite(&invite, Some(("Billing", &address)));
    invite
  }

  /// Turns join approval on once the cloud's account joined the server from another computer, whose data stays in
  /// `folder`: the server knows the account, so a join of it from any computer now waits for approval.
  pub fn ask_for_approval(&self, folder: &Path) {
    let first = self.server.email_invite(EMAIL);
    self.cloud.invite(&first, None);
    let workspace = Workspace::new(AppDb::open(&folder.join("orchester.db")).expect("the other computer's data"));
    let vault = Vault::with(std::sync::Arc::new(QaVault::default()));
    let servers = Servers::new(workspace.clone(), vault.clone());
    let cloud = Cloud::new(workspace, vault, servers);
    let runtime = Runtime::new().expect("runtime");
    runtime.block_on(async {
      let address = ServerAddress::parse(&self.cloud.url()).expect("cloud address");
      cloud.set_address(Some(address)).await.expect("the stand-in cloud");
      cloud
        .sign_in(EMAIL.into(), Credential::new(PASSWORD.into()))
        .await
        .expect("signed in");
      let listed = cloud.invites().await.expect("invites");
      let invite = listed
        .iter()
        .find(|invite| invite.id.to_string() == first)
        .expect("the invite");
      cloud.accept(invite).await.expect("joined from the other computer");
    });
    self.server.set_join_approval(true);
  }
}
