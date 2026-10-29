//! The window's state filled by the real `orchester-runner` and this computer's browser: the runner's connection
//! follows the project's browser once it attached, the person's tab appears, and a frame of a page reaches the window
//! as RGBA; the state goes with the runner.

use std::{fs, path::Path};

use orchester_ipc::{
  SessionEnd::Clean,
  browser::{BrowserCommand, BrowserReply, TabOwner, Viewport},
};
use orchester_workspace::{
  activity::Activity,
  runner::{QUIET_STOP_TIMEOUT, Runners, state::RunnerState},
  workspace::OpenProject,
};
use tempfile::TempDir;
use tokio::{runtime::Handle, time::timeout};

use super::{PATIENCE, near, runner_programs::programs, view_until};

/// A page of one color, filling the page area.
const PAGE: &str = "data:text/html,<html><body style=\"margin:0;background:rgb(0,128,255)\"></body></html>";

/// Whether this computer has a browser; where it has none, the test says so and checks nothing.
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no browser is installed"
)]
fn browser_installed() -> bool {
  match orchester_exec::browser::find() {
    Ok(_) => true,
    Err(unavailable) => {
      eprintln!("skipped, no browser to test with: {unavailable}");
      false
    }
  }
}

fn open(root: &Path) -> OpenProject {
  OpenProject {
    project: orchester_core::project::Project::new(
      orchester_core::id::ProjectId::generate(),
      orchester_core::project::ProjectKey::new("ORC").expect("key"),
      orchester_core::name::Name::new("Kontur").expect("name"),
    ),
    repo_root: root.to_path_buf(),
  }
}

fn project() -> (TempDir, OpenProject) {
  let folder = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(folder.path().join(".git")).expect("create .git");
  let open = open(folder.path());
  drop(orchester_store::ProjectDb::create(folder.path(), open.project.clone()).expect("create project"));
  (folder, open)
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_window_follows_the_runners_browser_and_its_frames() {
  if !browser_installed() {
    return;
  }
  let (folder, project) = project();
  let root = folder.path();
  let runners = Runners::new(programs().runner.clone(), Handle::current(), Activity::new(), Clean);
  let window = runners.browser_window();
  let mut updates = runners.subscribe();
  runners.show(Some(&project));
  timeout(
    PATIENCE,
    updates.wait_for(|status| status.state_for(root) == RunnerState::Online),
  )
  .await
  .expect("the runner online in time")
  .expect("the runners outlive the test");
  let view = view_until(&window, root, |_| true).await;
  assert_eq!(
    (view.name(), view.tabs()),
    ("Kontur", &[][..]),
    "followed once attached"
  );

  let opened = runners
    .browser(
      root,
      BrowserCommand::OpenTab {
        url: Some(PAGE.to_owned()),
      },
    )
    .await
    .expect("the person's tab opens");
  let BrowserReply::TabOpened { tab } = opened else {
    panic!("not a tab: {opened:?}");
  };
  let view = view_until(&window, root, |view| view.tabs().iter().any(|listed| listed.id == tab)).await;
  assert_eq!(view.tabs()[0].owner, TabOwner::Person);

  let viewport = Viewport::new(320, 200, 100).expect("a viewport within bounds");
  let shown = runners
    .browser(
      root,
      BrowserCommand::Show {
        tab: tab.clone(),
        viewport,
      },
    )
    .await
    .expect("the tab is shown");
  assert_eq!(shown, BrowserReply::Done);
  let view = view_until(&window, root, |view| {
    view
      .frame()
      .is_some_and(|frame| near(frame.pixels(), 100 * 320 + 160, [0, 128, 255]))
  })
  .await;
  let frame = view.frame().expect("a frame of the page");
  assert_eq!(
    (frame.width(), frame.height(), frame.css_size()),
    (320, 200, (320, 200))
  );
  assert_eq!((frame.tab(), view.shown()), (&tab, Some(&tab)));

  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
  let mut changes = window.changes();
  timeout(PATIENCE, changes.wait_for(|_| window.view(root).is_none()))
    .await
    .expect("the browser goes with the runner")
    .expect("the window's state outlives the test");
}
