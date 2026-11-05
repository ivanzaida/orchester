//! A task's Services tab (ORC-265, `cjvRr`) on the QA seed's `run-services` scenario with the real runner,
//! `orchester-fake-codex` and `orchester-test-command` as the services, which the test builds itself
//! (`support::runner_programs`): once the task runs, the tab lists the stack's services and cloud-web with their
//! states, counts them with a dot, follows the selected one's log live, shows team-server's crash and team-web stopped
//! for it, and its Stop and Restart go to the runner, which stops and starts the service again. The services' secret
//! values never reach the tab: the runner replaces them before it keeps a line, as its own test of the same request
//! shows (`crates/runner/tests/run_services.rs`); a real runner reads secrets only from the OS keychain, which a test
//! does not write. Needs `git`. Run with `cargo test -p orchester-app --test run_services run_services`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::{fs, path::Path, time::Duration};

use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{project::TaskKey, time::Timestamp};
use support::{Data, Window, runner_programs::programs};

const TITLE: &str = "Invite people to a team server";
/// What a live run of the fake and the services may take: the stack starts in seconds, team-server crashes after
/// about 45.
const PATIENCE: Duration = Duration::from_secs(90);

fn shows(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

fn says(window: &mut Window, id: &str) -> String {
  window.texts(id).unwrap_or_default().join(" ")
}

/// The window on the seeded project's task, its run started, on its Services tab; and the project's folder.
fn running() -> (Window, std::path::PathBuf) {
  let programs = programs();
  let data = Data::new();
  let repo = data.dir.path().join("repo-run-services");
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: kontur::scenarios::named("run-services").expect("a known scenario"),
    fake_codex: Some(programs.fake_codex.clone()),
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let launch = data.launch_with(programs.runner.clone());
  let mut window = Window::of(data, launch);
  let task = seeded.tasks.iter().find(|task| task.title == TITLE).expect("the task");
  let key: TaskKey = task.key.parse().expect("a task key");
  let page = |view| Route::Project(seeded.project, ProjectPage::Task(key.clone(), view));
  window.navigation().go(page(TaskView::of(TaskTab::Summary)));
  window.until_within(PATIENCE, "the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|element| element.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  window.press("task-run");
  window.navigation().go(page(TaskView::of(TaskTab::Services)));
  (window, repo)
}

/// Lets the run's step end: the fake holds its services while `fake-hold` lies next to the worktrees.
fn release(repo: &Path) {
  fs::remove_file(repo.join(".orchester").join("fake-hold")).expect("release the hold");
}

#[test]
fn run_services_tab_follows_the_runs_services_and_stops_and_restarts_them_through_the_runner() {
  let (mut window, repo) = running();
  // The log follows live: team-server's first lines as soon as it starts, then, with no press, its crash's.
  let deadline = std::time::Instant::now() + PATIENCE;
  while !shows(&mut window, "task-service-team-server") {
    window.tree.tick_futures();
    window.pass();
    if std::time::Instant::now() > deadline {
      use orchester_core::store::{ActivityStore, RunStore, TaskStore};
      let db = orchester_store::ProjectDb::open(&repo).expect("open");
      let mut seen = Vec::new();
      let runs: Vec<_> = db
        .tasks()
        .expect("tasks")
        .iter()
        .flat_map(|task| db.runs_of_task(task.id()).expect("runs"))
        .collect();
      for run in runs.iter().flatten() {
        seen.push(format!("{:?}", run.status()));
        for entry in db.activity(run.id(), 0).expect("activity") {
          seen.push(format!("{:?}", entry.event));
        }
      }
      window.navigation().back();
      window.pass();
      panic!(
        "DEBUG {}\nHEADER {}\nPAGE {}",
        seen.join("\n"),
        says(&mut window, "task-header"),
        says(&mut window, "screen-task")
      );
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  // On its name: the middle of a ready row is its address, which opens in the Browser tab.
  window.press("task-service-team-server-name");
  window.until_within(PATIENCE, "team-server's first lines", |window| {
    says(window, "task-services-log-lines").contains("INFO listening on 127.0.0.1:")
  });
  assert!(!says(&mut window, "task-services-log-lines").contains("ERROR accept invite"));
  window.until_within(PATIENCE, "the stack and cloud-web", |window| {
    ["postgres", "cloud-api", "team-server", "team-web", "cloud-web"]
      .iter()
      .all(|service| shows(window, &format!("task-service-{service}")))
  });
  assert_eq!(says(&mut window, "task-tab-services-count"), "5", "the tab counts them");
  assert_eq!(says(&mut window, "task-service-cloud-web-state"), "Starting");
  assert!(
    says(&mut window, "task-service-cloud-web-address").starts_with("Waiting for HTTP 2xx on /"),
    "a starting service says what it waits for"
  );
  assert!(
    says(&mut window, "task-services-stack-title").starts_with("Stack team"),
    "the stack line names the stack"
  );
  window.until_within(PATIENCE, "team-server's crash", |window| {
    says(window, "task-service-team-server-state") == "Crashed"
      && says(window, "task-services-log-lines").contains("ERROR accept invite")
  });
  assert!(says(&mut window, "task-service-team-server-address").starts_with("Exited with code 101 at"));
  window.until("team-web stopped for it", |window| {
    says(window, "task-service-team-web-address") == "Stopped: it needs team-server"
  });
  assert!(shows(&mut window, "task-tab-services-dot"), "a crash dots the tab");

  // Stop and Restart go to the runner, whose listing then says what it did.
  window.press("task-service-cloud-api-stop");
  window.until_within(PATIENCE, "cloud-api stopped by the runner", |window| {
    says(window, "task-service-cloud-api-state") == "Stopped"
  });
  window.press("task-service-team-server-restart");
  window.until_within(PATIENCE, "team-server restarted by the runner", |window| {
    says(window, "task-service-team-server-state") == "Ready"
  });
  assert!(
    says(&mut window, "task-services-stack-detail").contains("team-server by you at"),
    "the stack line says who restarted it"
  );

  // Stop on cloud-web, which still starts, gives its start up; then the step lets its services go and ends.
  window.press("task-service-cloud-web-stop");
  window.until_within(PATIENCE, "cloud-web's start given up", |window| {
    says(window, "task-service-cloud-web-state") == "Stopped"
  });
  release(&repo);
  window.until_within(PATIENCE, "the run's step to end", |window| {
    shows(window, "task-services-idle")
  });
}
