//! The agent field beside *Send to Implement* in the review's tray and beside *Ask the agent to resolve* in its
//! conflict bar (ORC-888, `Review - v2` 12 `IfAmk`, 02 `gSMiI`, 11 `CT6Gc`), in the app's real window on the QA seed's
//! `review-page` and `review-conflicts` scenarios: the field starts on the run's own agent, its menu lists the
//! project's published agent profiles with the run's own marked, choosing one sets the field alone, and the plain
//! button beside it sends with the profile chosen. That is seen from a profile the choice names whose version is no
//! longer one this computer uses by the time the button is pressed: the send is refused for it, where the run's own
//! agent is not refused. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_agent review_agent`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use rusqlite::Connection;

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diff with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// The model the seed gives every profile of these scenarios.
const MODEL: &str = "gpt-5.5";

struct Page {
  window: Window,
  _stand_ins: StandIns,
  project: ProjectId,
  repo: PathBuf,
  tasks: Vec<SeededTask>,
}

impl Page {
  /// The seed's `scenario` in a window whose project has no runner, opened on the review of `title`.
  fn open(scenario: &str, title: &str) -> Self {
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch_with(data.no_runner());
    let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    window.tree.resize(1440, 1400);
    let mut page = Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      repo: seeded.repo,
      tasks: seeded.tasks,
    };
    let task = page
      .tasks
      .iter()
      .find(|task| task.title == title)
      .unwrap_or_else(|| panic!("the scenario seeds {title}"));
    let key: TaskKey = task.key.parse().expect("a task key");
    let route = Route::Project(page.project, ProjectPage::TaskReview(key));
    page.window.navigation().go(route);
    page.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-diff").is_some()
    });
    page
  }

  /// Opens the menu of the field `id` and waits for its rows.
  fn open_menu(&mut self, id: &str) {
    self.window.press(id);
    let menu = format!("{id}-menu");
    self
      .window
      .until("the agent menu", |window| window.texts(&menu).is_some());
  }

  /// Chooses the profile named `name` in the menu of the field `id`; the menu closes.
  fn choose(&mut self, id: &str, name: &str) {
    self.open_menu(id);
    let mut at = 0;
    let row = loop {
      let row = format!("{id}-menu-{at}");
      assert!(self.window.texts(&row).is_some(), "{name} in the menu");
      if self.window.text(&format!("{row}-label")) == name {
        break row;
      }
      at += 1;
    };
    self.window.press(&row);
    self.window.pass();
  }

  /// Makes the Reviewer's published version one this computer does not use (a teammate's, waiting for *Use here*), the
  /// way a profile can stop being usable after the page listed it. The page keeps listing it until it reads again.
  fn stop_using_the_reviewer(&self) {
    let database = Connection::open(self.repo.join(".orchester").join("orchester.db")).expect("the project database");
    // The table names its version's shared body, which a project that is not shared has none of.
    database.execute_batch("PRAGMA foreign_keys = OFF").expect("a pragma");
    let waiting = database
      .execute(
        "INSERT INTO waiting_versions (version_id) SELECT id FROM resource_versions \
         WHERE resource_id = (SELECT id FROM resources WHERE kind = 'agent_profile' AND name = 'Reviewer')",
        [],
      )
      .expect("the version waits");
    assert_eq!(waiting, 1, "the Reviewer has one published version");
  }
}

#[test]
fn review_agent_tray_field_starts_on_the_runs_own_agent_and_lists_the_published_profiles() {
  let mut page = Page::open("review-page", "Sign webhook payloads");
  assert_eq!(
    page.window.text("review-tray-agent-label"),
    format!("Builder · {MODEL}")
  );
  assert_eq!(page.window.text("review-tray-send"), "Send to Implement (2)");
  page.open_menu("review-tray-agent");
  let menu = page.window.text("review-tray-agent-menu");
  assert!(menu.starts_with("Implement runs with"), "{menu}");
  for profile in ["Builder", "Explorer", "Planner", "Reviewer"] {
    assert!(menu.contains(profile), "{profile} in {menu}");
  }
  assert_eq!(
    page.window.text("review-tray-agent-menu-0-detail"),
    format!("{MODEL} · this run's agent"),
    "the run's own is told in its row"
  );
  assert_eq!(page.window.text("review-tray-agent-menu-3-detail"), MODEL);
  assert!(
    menu.ends_with("Only this time: the task's pipeline keeps its agents."),
    "{menu}"
  );
}

#[test]
fn review_agent_tray_choosing_a_profile_sets_the_field_and_sends_nothing() {
  let mut page = Page::open("review-page", "Sign webhook payloads");
  page.choose("review-tray-agent", "Reviewer");
  assert_eq!(
    page.window.text("review-tray-agent-label"),
    format!("Reviewer · {MODEL}")
  );
  assert!(page.window.texts("review-tray-agent-menu").is_none(), "the menu closed");
  assert_eq!(page.window.text("review-tray-title"), "2 comments not sent");
  assert_eq!(page.window.text("review-tray-send"), "Send to Implement (2)");
  page.choose("review-tray-agent", "Builder");
  assert_eq!(
    page.window.text("review-tray-agent-label"),
    format!("Builder · {MODEL}"),
    "the run's own again"
  );
}

#[test]
fn review_agent_tray_send_runs_the_step_with_the_profile_chosen_and_the_runs_own_by_default() {
  let mut page = Page::open("review-page", "Sign webhook payloads");
  page.choose("review-tray-agent", "Reviewer");
  page.stop_using_the_reviewer();
  page.window.press("review-tray-send");
  page.window.until_within(GIT_WORK, "the refusal", |window| {
    window.texts("review-not-done").is_some()
  });
  let refusal = page.window.text("review-not-done");
  assert!(refusal.contains("no published version"), "{refusal}");
  assert_eq!(
    page.window.text("review-tray-title"),
    "2 comments not sent",
    "the comments are kept unsent"
  );
}

#[test]
fn review_agent_tray_send_with_the_runs_own_agent_is_not_refused_for_a_profile() {
  let mut page = Page::open("review-page", "Sign webhook payloads");
  page.stop_using_the_reviewer();
  page.window.press("review-tray-send");
  page.window.until_within(GIT_WORK, "the send refused", |window| {
    window
      .texts("review-tray-title")
      .is_some_and(|title| title.join(" ") == "Not sent: the runner is not running")
  });
  assert!(page.window.texts("review-not-done").is_none());
}

#[test]
fn review_agent_conflict_bar_field_starts_on_the_runs_own_agent_and_resolve_passes_the_choice() {
  let mut page = Page::open("review-conflicts", "Rotate API keys from settings");
  assert_eq!(
    page.window.text("review-conflict-agent-label"),
    format!("Builder · {MODEL}")
  );
  assert_eq!(page.window.text("review-conflict-resolve"), "Ask the agent to resolve");
  page.choose("review-conflict-agent", "Reviewer");
  assert_eq!(
    page.window.text("review-conflict-agent-label"),
    format!("Reviewer · {MODEL}")
  );
  page.stop_using_the_reviewer();
  page.window.press("review-conflict-resolve");
  page.window.until_within(GIT_WORK, "the refusal", |window| {
    window.texts("review-not-done").is_some()
  });
  let refusal = page.window.text("review-not-done");
  assert!(refusal.contains("no published version"), "{refusal}");
  assert_eq!(page.window.text("review-status-text"), "Conflicts with main");
}

#[test]
fn review_agent_conflict_resolve_with_the_runs_own_agent_reaches_the_runner() {
  let mut page = Page::open("review-conflicts", "Rotate API keys from settings");
  page.stop_using_the_reviewer();
  page.window.press("review-conflict-resolve");
  page.window.until_within(GIT_WORK, "the refusal", |window| {
    window.texts("review-not-done").is_some()
  });
  let refusal = page.window.text("review-not-done");
  assert!(refusal.contains("runner"), "{refusal}");
  assert!(!refusal.contains("no published version"), "{refusal}");
}
