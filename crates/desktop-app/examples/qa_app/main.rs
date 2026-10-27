//! The app as `tools/qa-app.ps1` starts it for QA and design verification: the app's own window (`window::open`, as
//! `main` opens it) on the data beside this program, past the sign-in gate (2026-10-16). It signs in as a person would,
//! to a stand-in Orchester Cloud on loopback that answers while the window is open, with the device token in a keychain
//! in memory: nothing reaches the OS keychain and no QA copy talks to the real cloud. `--signed-out` stops at the gate,
//! with the stand-in set as the cloud (Advanced) so a person can sign in there. `--settings` (the `settings`
//! scenario) also sets up the team servers, invites and installed app's MCP rows app Settings shows ([`settings`]).
//! What it moves to the Recycle Bin / Trash goes to `trash` beside its data. Fixture data for QA, never shipped: an
//! example is no part of the app's program.
//!
//!   qa_app [--signed-in | --signed-out] [--settings]

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod agents;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
mod settings;
#[path = "../../tests/support/signed_in.rs"]
mod signed_in;

use std::{error::Error, fs, sync::Arc};

use orchester_api::client::ServerAddress;
use orchester_app::{
  app_log, clipboard,
  startup::{Launch, WindowPosition},
  window,
};
use orchester_workspace::{accounts::trash::FolderTrash, app_paths};
use signed_in::account_cloud::AccountCloud;
use tokio::runtime::Runtime;

fn main() -> Result<(), Box<dyn Error>> {
  let log = app_log::install();
  let mut signed_out = false;
  let mut with_settings = false;
  for argument in std::env::args().skip(1) {
    match argument.as_str() {
      "--signed-in" => signed_out = false,
      "--signed-out" => signed_out = true,
      "--settings" => with_settings = true,
      other => return Err(format!("expected --signed-in, --signed-out or --settings, not {other}").into()),
    }
  }
  let position = WindowPosition::from_environment()?;
  let location = app_paths::app_data_location()?;
  if let Some(folder) = location.database.parent() {
    fs::create_dir_all(folder)?;
  }
  let runtime = Runtime::new()?;
  let launch = Launch::begin_with_vault(
    &location.database,
    app_paths::runner_path()?,
    runtime.handle(),
    (log, signed_in::keychain()),
  );
  // What the QA copy moves to the Recycle Bin / Trash (a removed account's folder, discarded local changes) goes to
  // `trash` beside its data instead, never to the person's own.
  if let (Some(workspace), Some(folder)) = (launch.workspace(), location.database.parent()) {
    workspace.use_trash(Arc::new(FolderTrash::new(&folder.join("trash"))));
  }
  let cloud = AccountCloud::start();
  if let Some(services) = launch.services() {
    if signed_out {
      let address = ServerAddress::parse(&cloud.url())?;
      runtime.block_on(services.cloud.set_address(Some(address)))?;
    } else {
      signed_in::sign_in(services, runtime.handle(), &cloud);
    }
  }
  let prepared = match (with_settings && !signed_out, launch.services(), launch.workspace()) {
    (true, Some(services), Some(workspace)) => Some(settings::prepare(
      (services, workspace),
      (&runtime, &location.database),
      &cloud,
    )?),
    (true, ..) => return Err("--settings needs a started session".into()),
    (false, ..) => None,
  };
  println!(
    "cloud {} as {}\nsigned in {}\nsettings fixtures {}",
    cloud.url(),
    signed_in::account_cloud::EMAIL,
    !signed_out,
    prepared.is_some()
  );
  let (fixtures, shown) = prepared.map_or((None, None), |(fixtures, shown)| (Some(fixtures), Some(shown)));
  window::open_showing(&runtime, &launch, position, shown);
  drop(fixtures);
  clipboard::clear_at_exit();
  launch.end();
  Ok(())
}
