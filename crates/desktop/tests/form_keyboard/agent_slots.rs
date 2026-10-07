//! Project settings › Limits › Agents working at once (ORC-80), with the real runner: the field reads
//! the stored count, refuses one outside 1 to 16 and saves a valid one when it is left, and the
//! runner takes it at once, as the status bar's Agents shows; saves close together leave the runner
//! with the count stored last. Run with
//! `cargo test -p orchester-desktop --test form_keyboard agent_slots`.

use std::{path::Path, thread, time::Duration};

use orchester_core::{
  limits::{AgentSlots, ProjectLimits},
  store::SettingsStore,
};
use orchester_desktop::tasks::usage::save_limits;
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{init_repository, programs, seed},
};

const AGENT_SLOTS: &str = "limits-agent-slots";

fn stored_limits(repo: &Path) -> ProjectLimits {
  ProjectDb::open(repo)
    .expect("open the project")
    .project_limits()
    .expect("limits")
}

fn status_agents(desktop: &mut Desktop, text: &str) -> bool {
  desktop.tree.get_element_by_id("status-agents").is_some() && shows(desktop, "status-agents", text)
}

fn erase(desktop: &mut Desktop, count: usize) {
  for _ in 0..count {
    desktop.key("Backspace", "Backspace");
  }
}

#[test]
fn agent_slots_save_from_the_limits_tab_and_reach_the_runner_at_once() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  desktop.wait_until("the runner reports its slots", |desktop| status_agents(desktop, "0/4"));
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for("settings-tab-limits");
  desktop.click("settings-tab-limits");
  desktop.settle("the stored limits", |tree| {
    tree
      .get_element_by_id_mut(AGENT_SLOTS)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| !input.value().is_empty())
  });
  assert_eq!(desktop.value(AGENT_SLOTS), "4");
  assert!(shows(&mut desktop, "settings-limits", "Agents working at once"));
  assert!(shows(&mut desktop, "settings-limits", "1 to 16 · default 4"));

  // A count outside 1 to 16 says so and saves nothing when the field is left.
  desktop.tab_to(AGENT_SLOTS, 10);
  erase(&mut desktop, 1);
  desktop.type_text("20");
  assert!(shows(
    &mut desktop,
    "settings-limits",
    "Enter a whole number from 1 to 16."
  ));
  desktop.tab();
  assert_eq!(stored_limits(desktop.repo()), ProjectLimits::DEFAULT);

  desktop.tab_to(AGENT_SLOTS, 10);
  erase(&mut desktop, 2);
  desktop.type_text("2");
  desktop.tab();
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved count", |_| {
    stored_limits(&repo).agent_slots().get().get() == 2
  });
  assert_eq!(
    stored_limits(&repo),
    ProjectLimits::DEFAULT.with_agent_slots(stored_limits(&repo).agent_slots())
  );
  desktop.wait_until("the runner uses two slots", |desktop| status_agents(desktop, "0/2"));
}

#[test]
fn agent_slots_saved_close_together_leave_the_runner_with_the_stored_count() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  desktop.wait_until("the runner reports its slots", |desktop| status_agents(desktop, "0/4"));
  let repo = desktop.repo().to_path_buf();
  let savers: Vec<_> = (1..=8)
    .map(|count| {
      let (runners, repo) = (desktop.runners.clone(), repo.clone());
      thread::spawn(move || {
        let limits = ProjectLimits::DEFAULT.with_agent_slots(AgentSlots::new(count).expect("in range"));
        save_limits(&runners, &repo, &limits).expect("saved");
      })
    })
    .collect();
  for saver in savers {
    saver.join().expect("the save finished");
  }
  let stored = format!("0/{}", stored_limits(&repo).agent_slots().get());
  desktop.wait_until("the runner uses the stored count", |desktop| {
    status_agents(desktop, &stored)
  });
  // No count handed over earlier arrives after it.
  thread::sleep(Duration::from_millis(500));
  desktop.pass();
  assert!(status_agents(&mut desktop, &stored), "the runner keeps {stored}");
  // A save that leaves the count as it is hands nothing over and changes nothing.
  let budget_only = stored_limits(&repo);
  save_limits(&desktop.runners, &repo, &budget_only).expect("saved");
  desktop.pass();
  assert!(status_agents(&mut desktop, &stored));
}
