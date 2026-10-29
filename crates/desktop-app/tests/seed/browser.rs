//! The `browser` scenarios reach Settings › Browser's three states as the QA app reads them, with the installed
//! browsers looked for in `browsers` beside the data only: found, none found, and another program chosen.

#![cfg(any(target_os = "macos", target_os = "windows"))]

use orchester_store::AppDb;
use orchester_workspace::workspace::{AgentsBrowser, Workspace};

use super::Seed;

/// What Settings › Browser shows for the scenario `name`.
fn shown(name: &str) -> AgentsBrowser {
  let seed = Seed::of(name);
  let database = &seed.seeded.app_database;
  let workspace = Workspace::new(AppDb::open(database).expect("the application database"));
  let browsers = database.parent().expect("the data folder").join("browsers");
  assert!(browsers.is_dir(), "{name} makes the folder the QA app looks in");
  workspace.look_for_browsers_in(&[browsers]);
  workspace.browser_settings().expect("read").browser
}

#[test]
fn seed_browser_scenarios_reach_found_not_found_and_chosen() {
  assert!(
    matches!(shown("browser"), AgentsBrowser::Found(program) if program.name() == "Google Chrome"),
    "browser"
  );
  assert!(
    matches!(shown("browser-not-found"), AgentsBrowser::NotFound(_)),
    "browser-not-found"
  );
  assert!(
    matches!(shown("browser-chosen"), AgentsBrowser::Chosen(program) if program.name() == "Chromium"),
    "browser-chosen"
  );
}
