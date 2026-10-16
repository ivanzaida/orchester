//! The team server and Orchester Cloud a QA copy of the app talks to for N1 (Settings › Orchester, Connect a server,
//! Sharing): a real `orchester-server` on loopback presenting its own certificate, with Alice as its admin and the
//! projects Kontur, Billing and Mobile app; and a stand-in for the cloud's account API that names that server and
//! holds an invite to Billing for its one account. Everything is fixture data for QA, never shipped.
//!
//!   cargo run -p orchester-app --example n1_backends -- <file> [plain] [approval|refuse]
//!
//! With `plain` the server stays on plain HTTP to this computer, which needs no check code: the way to the join step
//! for a QA copy, whose MCP clients cannot confirm a check code. With `approval` the server asks for approval of a
//! join of the cloud's account, which joined it once from another computer (*Waiting for approval*); with `refuse` it
//! asks too, and its admin refuses each join that waits, a second or so after it asks (*refused this computer*).
//! Without a person to sign in and accept in the QA copy, `examples/n1_qa` opens these states seeded.
//!
//! writes the addresses, the server's check code, an invite code to Kontur and the cloud's account to `<file>` as
//! JSON, then serves until it is stopped (by its pid).

#![allow(clippy::print_stdout, reason = "a QA helper says where it listens")]

#[path = "../../../workspace/tests/sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "../n1_qa/backends.rs"]
mod backends;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../secrets_qa/vault.rs"]
#[allow(dead_code, reason = "the secrets QA's delays and failures are not used here")]
mod vault;

use std::{path::PathBuf, thread, time::Duration};

use account_cloud::{EMAIL, PASSWORD};
use backends::Backends;
use serde_json::json;

fn main() {
  let mut args = std::env::args().skip(1);
  let out = PathBuf::from(args.next().expect("the file to write the backends to"));
  let options: Vec<String> = args.collect();
  let has = |option: &str| options.iter().any(|given| given == option);
  let backends = Backends::start(has("plain"));
  let code = backends.server.project_code(&backends.kontur);
  // The other computer's data, beside the file; it stays for as long as this runs.
  let other = tempfile::Builder::new()
    .prefix("n1-other-computer-")
    .tempdir_in(out.parent().unwrap_or(&PathBuf::from(".")))
    .expect("the other computer's folder");
  let approval = has("approval") || has("refuse");
  if approval {
    backends.ask_for_approval(other.path());
  }
  backends.invite_to_billing();
  let backends_file = json!({
    "server": backends.server.url(),
    "check_code": backends.pin.map(|pin| pin.check_code()),
    "invite_code": code.expose(),
    "cloud": backends.cloud.url(),
    "email": EMAIL,
    "password": PASSWORD,
    "approval": if has("refuse") { "refused" } else if approval { "asked" } else { "off" },
  });
  std::fs::write(&out, serde_json::to_string_pretty(&backends_file).expect("JSON")).expect("write the backends");
  println!("backends written to {}", out.display());
  loop {
    thread::sleep(Duration::from_secs(1));
    if has("refuse") {
      backends.server.refuse_waiting();
    }
  }
}
