//! The app as `tools/qa-app.ps1` starts it for QA and design verification: the app's own window (`window::open`, as
//! `main` opens it) on the data beside this program, past the sign-in gate (2026-10-16). It signs in as a person would,
//! to a stand-in Orchester Cloud on loopback that answers while the window is open, with the device token in a keychain
//! in memory: nothing reaches the OS keychain and no QA copy talks to the real cloud. `--signed-out` stops at the gate,
//! with the stand-in set as the cloud (Advanced) so a person can sign in there. `--settings` (the `settings`
//! scenario) also sets up the kept invites and installed app's MCP rows app Settings shows ([`settings`]).
//! `--join-team` starts a stand-in team server on loopback that the stand-in cloud names as the account's, with an
//! invite to one of its projects, and prints its address and codes, for *Join a team*. A marker the seed's `team-*`
//! scenarios leave beside the database ([`team::State`]) sets up a stand-in team server on loopback that answers while
//! the window is open ([`team`]); where a person's press is part of the state, the window opens there ([`team_window`]).
//! What it moves to the Recycle Bin / Trash goes to `trash` beside its data. Fixture data for QA, never shipped: an
//! example is no part of the app's program.
//!
//!   qa_app [--signed-in | --signed-out] [--settings] [--join-team]

#![allow(clippy::print_stdout, reason = "a QA helper says where it is")]

mod agents;
mod settings;
mod team;
mod team_window;

use std::{error::Error, fs, sync::Arc};

use orchester_api::test_support::team_server::{CODE, LATER_CODE, TeamServer};
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

/// The stand-in team server's id at the stand-in cloud, and its invite to Billing.
const TEAM: &str = "0191c7c0-0000-7000-8000-0000000005e1";
const INVITE: &str = "0191c7c0-0000-7000-8000-00000000171e";

fn main() -> Result<(), Box<dyn Error>> {
  let log = app_log::install();
  let mut signed_out = false;
  let mut with_settings = false;
  let mut join_team = false;
  for argument in std::env::args().skip(1) {
    match argument.as_str() {
      "--signed-in" => signed_out = false,
      "--signed-out" => signed_out = true,
      "--settings" => with_settings = true,
      "--join-team" => join_team = true,
      other => {
        return Err(format!("expected --signed-in, --signed-out, --settings or --join-team, not {other}").into());
      }
    }
  }
  let position = WindowPosition::from_environment()?;
  let location = app_paths::app_data_location()?;
  if let Some(folder) = location.database.parent() {
    fs::create_dir_all(folder)?;
  }
  let team_state = team::State::beside(&location.database)?;
  if signed_out && team_state.is_some() {
    return Err("a team scenario joins a team while signed in: it cannot go with --signed-out".into());
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
  if let (Some(_), Some(services)) = (&reports, launch.services()) {
    services
      .runners
      .send_error_reports(error_reports::for_runners(Environment::Qa));
  }
  // What the QA copy moves to the Recycle Bin / Trash (a removed account's folder, discarded local changes) goes to
  // `trash` beside its data instead, never to the person's own.
  if let (Some(workspace), Some(folder)) = (launch.workspace(), location.database.parent()) {
    workspace.use_trash(Arc::new(FolderTrash::new(&folder.join("trash"))));
  }
  let cloud = AccountCloud::start();
  // Kept to the end of `main`: the team server answers while the window is open.
  let _team = join_team.then(|| team(&cloud));
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
  // The team answers while the window is open.
  let team_server = match (team_state, launch.services()) {
    (Some(state), Some(services)) => Some(team::prepare(state, services, runtime.handle())?),
    (Some(_), None) => return Err("a team scenario needs a started session".into()),
    (None, _) => None,
  };
  println!(
    "cloud {} as {}\nsigned in {}\nsettings fixtures {}\nteam {}",
    cloud.url(),
    signed_in::account_cloud::EMAIL,
    !signed_out,
    shown.is_some(),
    team_server.as_ref().map_or_else(|| "none".to_owned(), TeamServer::url),
  );
  match (team_state.filter(|state| state.drives()), &team_server) {
    (Some(state), Some(server)) => team_window::open(&runtime, &launch, position, (state, server))?,
    _ => window::open_showing(&runtime, &launch, position, shown),
  }
  clipboard::clear_at_exit();
  launch.end();
  Ok(())
}

/// A stand-in team server that `cloud` names as the account's, with an invite to its project Billing; says where it
/// is and which codes it takes.
fn team(cloud: &AccountCloud) -> TeamServer {
  let team = TeamServer::start();
  cloud.name_server(TEAM, &team.url(), None);
  cloud.invite(INVITE, Some(("Billing", &format!("{}/billing", team.url()))));
  println!(
    "team server {}\ncode {CODE} joins, {LATER_CODE} joins until it expires; any other code is wrong",
    team.url()
  );
  team
}
