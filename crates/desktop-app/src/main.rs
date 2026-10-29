// A release build is a window without a console: started from Explorer or the Start menu, a console
// program opens a console beside it. Debug builds keep the console and its log output.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use orchester_app::{
  app_log, clipboard,
  error_reports::{self, Environment},
  startup::{Launch, WindowPosition},
  window,
};
use tokio::runtime::Runtime;

fn main() {
  let log = app_log::install();
  // Refused before anything opens in a QA copy or a debug build: a run that asked for a place must not get
  // a window elsewhere. The installed app ignores the variable (`WindowPosition::requested`).
  let position = WindowPosition::from_environment().unwrap_or_else(|error| {
    tracing::error!(%error, "the requested window position is refused; not starting");
    std::process::exit(2);
  });

  let runtime = Runtime::new().expect("the OS provides threads for the background runtime");
  // Started before the window exists: the data folder's lock, the session, the runners' client and
  // where the window opens, or why there is no session.
  let mut launch = Launch::begin(runtime.handle(), log);
  // Error reports start here, once the database that holds the person's setting is open: a panic before this line is
  // not reported. Kept to the end of `main`, whose drop sends what is still pending. The runners, which start once
  // the window shows a project, report too.
  let environment = Environment::of_this_app();
  let reports = environment.and_then(|environment| error_reports::start(environment, launch.workspace()));
  if let (Some(environment), Some(_), Some(services)) = (environment, &reports, launch.services()) {
    services
      .runners
      .set_error_reports(Some(&error_reports::for_runners(environment)));
  }
  // Only this window moves a removed account's folder to the person's Recycle Bin / Trash; tests and QA copies have
  // a trash of their own.
  if let Some(workspace) = launch.workspace() {
    workspace.use_trash(std::sync::Arc::new(orchester_workspace::accounts::trash::SystemTrash));
  }
  // Only this window reads the accounts' plan limits on its own; tests and QA copies never read a real account.
  launch.read_plan_limits();
  window::open(&runtime, &launch, position);
  // The window has closed and its server stopped: then the runners stop and the session records its end.
  clipboard::clear_at_exit();
  launch.end();
}
