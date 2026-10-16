//! This computer's Orchester Cloud account and what it leads to (`docs/architecture/server.md`, "Orchester Cloud",
//! "Invites", rows 39, 43, 49–50): signing in and out, invites accepted with the certificate pin the invite carries (no
//! check code to compare) and declined, a cloud that does not deliver invites yet, joining a known server with the
//! account, a code's identity token shown only to the server holding the key of its id, a pinned server that presents
//! another certificate, and the linked projects signing out names.
//!
//! `cargo test -p orchester-workspace --test cloud_account cloud_account_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "sharing_support/impostor.rs"]
mod impostor;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use desktop::{Desktop, linked_server};
use impostor::{Claimed, Impostor};
use orchester_api::{
  Credential,
  client::{Client, ServerAddress},
  cloud::CloudErrorCode,
};
use orchester_core::{repository::RepositoryIdentity, secret::SecretVault};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{
  CloudJoined, CodeRequest, Connection, PresentedCertificate, RemoteMatch, Servers, SharingError,
  cloud::{CloudError, ServerJoin, WebPage},
};
use server_support::TestServer;

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
  let accepted = desktop.block_on(desktop.cloud.accept(&invites[0])).expect("accept");
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
  assert!(
    desktop
      .block_on(desktop.cloud.accepted_invites())
      .expect("kept")
      .is_empty(),
    "a join that finished keeps nothing"
  );
}

/// A server is set up after it starts presenting its own certificate, as `n1_backends` does in TLS mode (N1 design 3),
/// and its invite is accepted with the pin the cloud names.
#[test]
fn cloud_account_a_server_on_its_own_certificate_is_set_up_and_joined() {
  let cloud = AccountCloud::start();
  let (mut server, audience) = linked_server(&cloud);
  server.serve_self_signed();
  let billing = server.create_named_project(("Billing", "BIL"), "billing", None);
  let invite = server.project_email_invite(EMAIL, &billing);
  server.set_join_approval(false);
  let desktop = Desktop::signed_in(&cloud);
  let address = ServerAddress::parse(&server.url()).expect("address");
  let pin = desktop
    .block_on(Client::presented_certificate(&address))
    .expect("the server's certificate");
  cloud.name_server(&audience, &server.url(), Some(&pin.to_hex()));
  cloud.invite(&invite, Some(("Billing", &format!("{}/billing", server.url()))));
  let invites = desktop.block_on(desktop.cloud.invites()).expect("invites");
  let accepted = desktop.block_on(desktop.cloud.accept(&invites[0])).expect("accept");
  assert!(matches!(accepted.joined, CloudJoined::Joined(_)), "{accepted:?}");
  assert_eq!(accepted.project.as_deref(), Some("Billing"));
}

#[test]
fn cloud_account_declining_an_invite_ends_it() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  let invite = server.email_invite(EMAIL);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, None);
  let desktop = Desktop::signed_in(&cloud);
  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites").remove(0);
  desktop.block_on(desktop.cloud.decline(listed.id)).expect("decline");
  assert_eq!(cloud.declined(), vec![invite]);
  assert!(
    desktop.servers.entries().expect("entries").is_empty(),
    "nothing was joined"
  );
  let again = desktop.block_on(desktop.cloud.accept(&listed));
  assert!(matches!(again, Err(CloudError::InviteEnded)), "{again:?}");
  assert!(
    desktop
      .block_on(desktop.cloud.accepted_invites())
      .expect("kept")
      .is_empty(),
    "an invite that ended is not kept"
  );
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
fn cloud_account_a_code_shows_its_identity_only_to_the_server_holding_the_key_of_its_id() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::new(&cloud.url());
  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("sign in");
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let id = server.cloud_audience();
  server.serve_self_signed();
  let pinned = |url: &str| {
    let address = ServerAddress::parse(url).expect("address");
    let trust = desktop
      .block_on(PresentedCertificate::read(&address))
      .expect("certificate")
      .confirm();
    (address, trust)
  };
  let nonce = orchester_api::identity::key_proof_nonce().expect("a nonce");
  let public_key = server
    .block_on(server.client().key_proof(&nonce))
    .expect("the server's key")
    .public_key;
  let request = |code: Credential| CodeRequest {
    code,
    username: Some("bohdan".into()),
    display_name: Some("Bohdan Koval".into()),
    proof: None,
  };
  // A server that gives itself this server's id: with a key of its own, with this server's key it cannot sign with,
  // or passing the nonce on to this server and returning its proof, which names this server's certificate.
  let claims = [
    Claimed::OwnKey,
    Claimed::Borrowed(public_key),
    Claimed::Relayed(server.client()),
  ];
  for claimed in claims {
    let impostor = Impostor::start(&id, claimed);
    let (address, trust) = pinned(&impostor.url());
    let code = Credential::new("ABCD-EFGH".into());
    let refused = desktop.block_on(desktop.cloud.join_with_code(&address, trust, request(code)));
    assert!(
      matches!(refused, Err(CloudError::Join(SharingError::UnprovenServer))),
      "{refused:?}"
    );
    assert!(
      !impostor.asked().iter().any(|asked| asked.contains("/auth/code")),
      "no code was sent: {:?}",
      impostor.asked()
    );
  }
  // The server itself proves its key on the connection this computer checked, and takes the code with the token.
  let (address, trust) = pinned(&server.url());
  desktop
    .block_on(
      desktop
        .cloud
        .join_with_code(&address, trust, request(server.code_for("bohdan", false))),
    )
    .expect("joined with the code and the token");
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
    proof: Some(server.code_proof("bohdan")),
  };
  let id = desktop
    .block_on(desktop.servers.join_with_code(&address, presented.confirm(), request))
    .expect("join with a code");
  // As if the server had made a new certificate: this computer's pin no longer matches it.
  desktop.pin_server(id, &"ab".repeat(32));
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
