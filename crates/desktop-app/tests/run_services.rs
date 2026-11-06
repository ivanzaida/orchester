//! A task's Services tab (ORC-265, `cjvRr`) on the QA seed's `run-services` scenario with the real runner,
//! `orchester-fake-codex` and `orchester-test-command` as the services, which the test builds itself
//! (`support::runner_programs`): once the task runs, the tab lists the stack's services and cloud-web with their
//! states, counts them with a dot, follows the selected one's log live, shows team-server's crash and team-web stopped
//! for it, and its Stop and Restart go to the runner, which stops and starts the service again; an MCP client's
//! synthetic press on Restart starts nothing and the tab says only a person starts it. The services' secret
//! values never reach the tab: the runner replaces them before it keeps a line, as its own test of the same request
//! shows (`crates/runner/tests/run_services.rs`); a real runner reads secrets only from the OS keychain, which a test
//! does not write.
//!
//! On `run-services-approve` the agent's start of team-web asks the person in one card for it and what it needs
//! (ORC-275, `hvQDS`), and each answer goes to the runner: *Approve* starts them, *Approve for every run while
//! unchanged*, confirmed in its modal, keeps a rule of the project per service, *Deny* tells the agent no. Needs
//! `git`. Run with `cargo test -p orchester-app --test run_services run_services`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod support;

use std::{fs, path::Path, time::Duration};

use lurq::{app::events::with_synthetic_input, node::element::ElementRef};
use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{project::TaskKey, time::Timestamp};
use orchester_store::ProjectDb;
use support::{Data, Window, runner_programs::programs};

const TITLE: &str = "Invite people to a team server";
/// The task of `run-services-approve`.
const APPROVE_TITLE: &str = "Start the team web app";
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
  running_on(("run-services", TITLE), TaskTab::Services)
}

/// The window on the task titled `title` of `scenario`, its run started, on its `tab`; and the project's folder.
fn running_on((scenario, title): (&str, &str), tab: TaskTab) -> (Window, std::path::PathBuf) {
  let programs = programs();
  let data = Data::new();
  let repo = data.dir.path().join("repo-run-services");
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
    fake_codex: Some(programs.fake_codex.clone()),
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let launch = data.launch_with(programs.runner.clone());
  let mut window = Window::of(data, launch);
  let task = seeded.tasks.iter().find(|task| task.title == title).expect("the task");
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
  window.navigation().go(page(TaskView::of(tab)));
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
  window.until_within(PATIENCE, "team-server", |window| {
    shows(window, "task-service-team-server")
  });
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
  // Restart is a person's press: an MCP client's input starts nothing, and the tab says so.
  with_synthetic_input(|| window.press("task-service-team-server-restart"));
  window.until("the refusal of a synthetic Restart", |window| {
    shows(window, "task-services-person-only")
  });
  assert_eq!(says(&mut window, "task-service-team-server-state"), "Crashed");
  window.press("task-service-team-server-restart");
  window.until("a person's press clears the refusal", |window| {
    !shows(window, "task-services-person-only")
  });
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

/// The id of the approve services card that waits in the Conversation, once its answers are laid out.
fn approval_card(window: &mut Window) -> String {
  let mut found: Option<String> = None;
  window.until_within(PATIENCE, "the approve services card", |window| {
    found = window.tree.get_element_by_id("conversation").and_then(|tab| {
      let mut card = None;
      walk(tab, &mut |element| {
        if let Some(id) = element.id().and_then(|id| id.strip_suffix("-every-run"))
          && id.starts_with("cards-approval-")
        {
          card = Some(id.to_owned());
        }
      });
      card
    });
    found.as_ref().is_some_and(|card| {
      window
        .tree
        .get_element_by_id_mut(&format!("{card}-approve"))
        .and_then(|element| element.bounds())
        .is_some()
    })
  });
  found.expect("the card")
}

fn walk<'a>(element: ElementRef<'a>, visit: &mut impl FnMut(ElementRef<'a>)) {
  visit(element);
  for child in element.children() {
    walk(child, visit);
  }
}

/// How the card `card` reads once its answer was taken.
fn decided(window: &mut Window, card: &str) -> String {
  let decided = format!("{card}-decided");
  window.until_within(PATIENCE, "the answer taken", |window| shows(window, &decided));
  says(window, &decided)
}

#[test]
fn run_services_approve_starts_the_service_and_its_needs_through_the_runner() {
  let (mut window, repo) = running_on(("run-services-approve", APPROVE_TITLE), TaskTab::Conversation);
  let card = approval_card(&mut window);
  assert_eq!(
    says(&mut window, &format!("{card}-title")),
    "Start service team-web and what it needs: team-server, cloud-api, postgres"
  );
  assert!(says(&mut window, &format!("{card}-program-team-server")).ends_with("changed: arguments"));
  assert!(says(&mut window, &format!("{card}-program-team-web")).ends_with("first start"));
  window.press(&format!("{card}-approve"));
  let said = decided(&mut window, &card);
  assert!(said.starts_with("Approved once · by you at "), "{said}");
  // The runner started what the person approved: the Services tab counts the four.
  window.until_within(PATIENCE, "the four started", |window| {
    says(window, "task-tab-services-count") == "4"
  });
  release(&repo);
}

#[test]
fn run_services_approve_for_every_run_keeps_a_rule_of_the_project_per_service() {
  let (mut window, repo) = running_on(("run-services-approve", APPROVE_TITLE), TaskTab::Conversation);
  let card = approval_card(&mut window);
  let confirm = format!("{card}-every-run-confirm-confirm");
  window.press(&format!("{card}-every-run"));
  window.until("the modal", |window| {
    window
      .tree
      .get_element_by_id_mut(&confirm)
      .and_then(|element| element.bounds())
      .is_some()
  });
  window.press(&confirm);
  let said = decided(&mut window, &card);
  assert!(said.starts_with("Approved for every run of this project"), "{said}");
  let rules = ProjectDb::open(&repo)
    .expect("open")
    .project_approval_rules()
    .expect("rules");
  let approved = |name: &str| {
    rules
      .iter()
      .filter(|rule| rule.is_active())
      .filter(|rule| rule.scope.service().is_some_and(|service| service.name == name))
      .count()
  };
  assert_eq!(approved("team-web"), 1, "team-web is approved for every run now");
  assert_eq!(approved("team-server"), 2, "team-server as it was and as it is now");
  release(&repo);
}

#[test]
fn run_services_deny_tells_the_agent_no_through_the_runner() {
  let (mut window, repo) = running_on(("run-services-approve", APPROVE_TITLE), TaskTab::Conversation);
  let card = approval_card(&mut window);
  window.press(&format!("{card}-deny"));
  let said = decided(&mut window, &card);
  assert!(said.starts_with("Denied · by you at "), "{said}");
  release(&repo);
}
