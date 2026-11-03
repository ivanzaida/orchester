//! A profile's *Extra writable folders* (S14 b `B6blpz`): its hint says reading needs nothing there, and its link opens
//! the project's Sources in Project settings › Context. `cargo test -p orchester-app --test agents writable_folders`.
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};

use super::{harness::Agents, support::project_id};

#[test]
fn writable_folders_say_where_reading_comes_from_and_open_the_sources() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  assert_eq!(
    screen.text("agent-folders-hint"),
    "Only with Edit files. Folders outside the worktree this agent may also change. Reading needs nothing here: \
     every agent reads the project’s Sources."
  );
  assert_eq!(
    screen.text("agent-folders-sources"),
    "Sources in Project settings › Context"
  );
  screen.click("agent-folders-sources");
  assert_eq!(
    screen.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::Context))
  );
}
