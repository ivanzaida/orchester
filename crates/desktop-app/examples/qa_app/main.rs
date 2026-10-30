//! The app as `tools/qa-app.ps1` starts it for QA and design verification: the app's own window (`window::open`, as
//! `main` opens it) on the data beside this program, past the sign-in gate (2026-10-16). It signs in as a person would,
//! to a stand-in Orchester Cloud on loopback that answers while the window is open, with the device token in a keychain
//! in memory: nothing reaches the OS keychain and no QA copy talks to the real cloud. `--signed-out` stops at the gate,
//! with the stand-in set as the cloud (Advanced) so a person can sign in there. `--settings` (the `settings`
//! scenario) also sets up the kept invites and installed app's MCP rows app Settings shows ([`settings`]).
//! What it moves to the Recycle Bin / Trash goes to `trash` beside its data. Fixture data for QA, never shipped: an
//! example is no part of the app's program.
//!
//!   qa_app [--signed-in | --signed-out] [--settings]

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod agents;
mod settings;

use std::{error::Error, fs, sync::Arc};

use orchester_app::{
  app_log, clipboard,
  error_reports::{self, Environment},
  startup::{Launch, WindowPosition},
  test_support::signed_in,
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
  // A QA copy reports its panics as `qa`, though it is a debug build (the person's decision, ORC-156), once its
  // database is open and while its setting allows; kept to the end of `main`. Its runners are told too, though a debug
  // runner never sends.
  let reports = error_reports::start(Environment::Qa, launch.workspace());
  if let (Some(reports), Some(services)) = (&reports, launch.services()) {
    services.runners.set_error_reports(reports.for_runners().as_ref());
  }
  // What the QA copy moves to the Recycle Bin / Trash (a removed account's folder, discarded local changes) goes to
  // `trash` beside its data instead, never to the person's own.
  if let (Some(workspace), Some(folder)) = (launch.workspace(), location.database.parent()) {
    workspace.use_trash(Arc::new(FolderTrash::new(&folder.join("trash"))));
  }
  let cloud = AccountCloud::start();
  if let Some(services) = launch.services() {
    if signed_out {
      let address = signed_in::cloud_address(&cloud);
      runtime.block_on(services.cloud.set_address(Some(address)))?;
    } else {
      signed_in::sign_in(services, runtime.handle(), &cloud);
    }
  }
  let shown = match (with_settings && !signed_out, launch.workspace()) {
    (true, Some(workspace)) => Some(settings::prepare(workspace, &location.database)?),
    (true, None) => return Err("--settings needs a started session".into()),
    (false, _) => None,
  };
  println!(
    "cloud {} as {}\nsigned in {}\nsettings fixtures {}",
    cloud.url(),
    signed_in::account_cloud::EMAIL,
    !signed_out,
    shown.is_some()
  );
  window::open_showing(&runtime, &launch, position, shown);
  clipboard::clear_at_exit();
  launch.end();
  Ok(())
}
