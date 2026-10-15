//! The team server and Orchester Cloud a QA copy of the app talks to for N1 (Settings › Orchester, Connect a server,
//! Sharing): a real `orchester-server` on loopback presenting its own certificate, with Alice as its admin and the
//! projects Kontur, Billing and Mobile app; and a stand-in for the cloud's account API that names that server and
//! holds an invite to Billing for its one account. Everything is fixture data for QA, never shipped.
//!
//!   cargo run -p orchester-app --example n1_backends -- <file> [plain]
//!
//! With `plain` the server stays on plain HTTP to this computer, which needs no check code: the way to the join step
//! for a QA copy, whose MCP clients cannot confirm a check code.
//!
//! writes the addresses, the server's check code, an invite code to Kontur and the cloud's account to `<file>` as
//! JSON, then serves until it is stopped (by its pid).

#![allow(clippy::print_stdout, reason = "a QA helper says where it listens")]

#[path = "../../../workspace/tests/sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;

use std::{path::PathBuf, thread, time::Duration};

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use orchester_api::client::{Client, ServerAddress};
use serde_json::json;
use server_support::TestServer;

fn main() {
  let out = PathBuf::from(std::env::args().nth(1).expect("the file to write the backends to"));
  let cloud = AccountCloud::start(true);
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
  let code = server.project_code(&kontur);
  let invite = server.project_email_invite(EMAIL, &billing);
  let plain = std::env::args().nth(2).as_deref() == Some("plain");
  let pin = if plain {
    None
  } else {
    server.serve_self_signed();
    let address = ServerAddress::parse(&server.url()).expect("address");
    Some(
      server
        .block_on(Client::presented_certificate(&address))
        .expect("the server's certificate"),
    )
  };
  cloud.name_server(&audience, &server.url(), pin.map(|pin| pin.to_hex()).as_deref());
  cloud.invite(&invite, Some(&format!("{}/billing", server.url())));
  let backends = json!({
    "server": server.url(),
    "check_code": pin.map(|pin| pin.check_code()),
    "invite_code": code.expose(),
    "cloud": cloud.url(),
    "email": EMAIL,
    "password": PASSWORD,
  });
  std::fs::write(&out, serde_json::to_string_pretty(&backends).expect("JSON")).expect("write the backends");
  println!("backends written to {}", out.display());
  loop {
    thread::sleep(Duration::from_secs(3600));
  }
}
