//! A team start that fails for good (ORC-885): on the QA state `team-start-failed`, where the server builds KON-5's
//! start in a bundle schema one newer than this computer reads, Run claims the task, the start fails for good, the
//! claim is released, and the task page says why instead of "could not be carried out on this computer". The other
//! reasons the stand-in can give say theirs, a second Run after a failure claims again, and a start that waited is
//! carried on when the project is opened again.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa team_qa_start_failed`

use std::time::Duration;

use orchester_api::test_support::team_server::{BundleFault, TeamServer};
use orchester_app::{test_support::Window, ui::icon::Icon};
use orchester_store::ProjectDb;

use super::{
  not_ready::{on_task, task_route},
  team::State,
};

/// KON-5 on `team-start-failed`, with the server's start failing as `fault` says (the state's own newer schema when
/// `None`), Run pressed once the runner shows, and the notice up.
fn run_pressed(fault: Option<BundleFault>) -> (Window, TeamServer) {
  run_pressed_on(|server| {
    if let Some(fault) = fault {
      server.authority_bundle_fault(fault);
    }
  })
}

/// [`run_pressed`] with the server set up by `setup` first.
fn run_pressed_on(setup: impl FnOnce(&TeamServer)) -> (Window, TeamServer) {
  let (mut window, server, _) = on_task(("team-start-failed", State::StartFailed), "KON-5");
  setup(&server);
  press_run(&mut window);
  window.until("the notice", |window| window.texts("task-team-refused").is_some());
  (window, server)
}

fn press_run(window: &mut Window) {
  window.until("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  window.press("task-run");
}

fn asked(server: &TeamServer, kind: &str) -> usize {
  server.commands().iter().filter(|asked| *asked == kind).count()
}

fn notice(window: &mut Window) -> String {
  window.texts("task-team-refused").expect("the notice").join(" ")
}

#[test]
fn team_qa_start_failed_says_the_bundle_schema_and_lets_the_claim_go() {
  let (mut window, server) = run_pressed(None);

  assert!(
    window.text("task-team-refused").contains("Couldn't start KON-5's run"),
    "{}",
    window.text("task-team-refused")
  );
  let body = notice(&mut window);
  assert!(body.contains("Update Orchester on this computer"), "{body}");
  assert!(
    body.contains("Nothing ran. Your claim on the task was released."),
    "{body}"
  );
  assert!(!body.contains("could not be carried out"), "{body}");
  assert_eq!(asked(&server, "work.start"), 1, "the server made the work once");
  assert_eq!(asked(&server, "claim.release"), 1, "the claim was let go of");
}

#[test]
fn team_qa_start_failed_a_server_that_refuses_to_release_running_work_says_the_claim_may_still_be_held() {
  let (mut window, server) = run_pressed_on(TeamServer::authority_refuses_running_releases);

  let body = notice(&mut window);
  assert!(body.contains("Update Orchester on this computer"), "{body}");
  assert!(
    body.contains("The team server refused to release your claim on the task, so it may still be held."),
    "{body}"
  );
  assert!(!body.contains("Your claim on the task was released"), "{body}");
  assert_eq!(asked(&server, "claim.release"), 1, "the release was asked for once");
}

#[test]
fn team_qa_start_failed_an_older_server_is_to_be_updated_by_whoever_runs_it() {
  let (mut window, server) = run_pressed(Some(BundleFault::Older));

  let body = notice(&mut window);
  assert!(body.contains("The team server is older than this app"), "{body}");
  assert!(body.contains("Your claim on the task was released."), "{body}");
  assert_eq!(asked(&server, "claim.release"), 1);
}

#[test]
fn team_qa_start_failed_a_bundle_this_app_cannot_read_says_to_update_them() {
  let (mut window, server) = run_pressed(Some(BundleFault::Unreadable));

  let body = notice(&mut window);
  assert!(body.contains("something this app can't read"), "{body}");
  assert!(body.contains("Your claim on the task was released."), "{body}");
  assert_eq!(asked(&server, "claim.release"), 1);
}

#[test]
fn team_qa_start_failed_a_bundle_the_server_cannot_build_is_missing_and_lets_the_claim_go() {
  let (mut window, server) = run_pressed(Some(BundleFault::Missing));

  let body = notice(&mut window);
  assert!(body.contains("could not build this run's bundle"), "{body}");
  assert_eq!(
    asked(&server, "claim.release"),
    1,
    "no work was made, so the claim was let go of"
  );
}

#[test]
fn team_qa_start_failed_a_second_run_claims_again_and_is_not_refused_for_the_old_claim() {
  let (mut window, server) = run_pressed(None);
  assert_eq!(asked(&server, "claim.acquire"), 1);

  press_run(&mut window);
  window.until("the second start", |_| asked(&server, "work.start") == 2);
  window.until("the notice again", |window| window.texts("task-team-refused").is_some());

  let body = notice(&mut window);
  assert!(body.contains("Update Orchester on this computer"), "{body}");
  assert_eq!(
    asked(&server, "claim.acquire"),
    2,
    "the second Run claimed the task again"
  );
  assert_eq!(asked(&server, "claim.release"), 2);
}

#[test]
fn team_qa_start_failed_a_start_that_waited_is_carried_on_when_the_project_is_opened_again() {
  let (mut window, server, project) = on_task(("team-start-failed", State::StartFailed), "KON-5");
  let root = window.data.repo("repo-kontur");
  server.authority_bundle_fault(BundleFault::Unavailable);
  press_run(&mut window);
  window.until("the start waits", |window| {
    window
      .texts("task-team-refused")
      .is_some_and(|texts| texts.join(" ").contains("starts once they answer"))
  });
  assert_eq!(
    ProjectDb::open(&root)
      .expect("open")
      .open_team_starts()
      .expect("open")
      .len(),
    1
  );

  // The server answers again, and the project is opened anew: its open start goes on without a press of Run.
  server.authority_bundle_fault(BundleFault::Newer);
  let Window { data, launch, .. } = window;
  let mut window = Window::of(data, launch);
  window.navigation().go(task_route(project, "KON-5"));
  window.until_within(Duration::from_secs(30), "the start is carried on", |_| {
    ProjectDb::open(&root)
      .expect("open")
      .open_team_starts()
      .expect("open")
      .is_empty()
  });
  assert_eq!(asked(&server, "claim.release"), 1, "the failed start let the claim go");
  assert_eq!(asked(&server, "work.start"), 1, "the same start went on, not a new one");
}

#[test]
fn team_qa_start_failed_a_run_that_waits_after_recovery_went_idle_is_carried_on_without_reopening() {
  let (mut window, server, _project) = on_task(("team-start-failed", State::StartFailed), "KON-5");
  let root = window.data.repo("repo-kontur");
  // Nothing was open when the project opened: the first look at its starts is over, and recovery idles.
  assert!(
    ProjectDb::open(&root)
      .expect("open")
      .open_team_starts()
      .expect("open")
      .is_empty()
  );
  server.authority_bundle_fault(BundleFault::Unavailable);
  press_run(&mut window);
  window.until("the start waits", |window| {
    window
      .texts("task-team-refused")
      .is_some_and(|texts| texts.join(" ").contains("starts once they answer"))
  });

  // The server answers again: the same window, with no new press of Run, carries the start on.
  server.authority_bundle_fault(BundleFault::Newer);
  window.until_within(Duration::from_secs(30), "the start is carried on", |_| {
    ProjectDb::open(&root)
      .expect("open")
      .open_team_starts()
      .expect("open")
      .is_empty()
  });
  assert_eq!(asked(&server, "claim.release"), 1, "the failed start let the claim go");
  assert_eq!(asked(&server, "work.start"), 1, "the same start went on, not a new one");
}
