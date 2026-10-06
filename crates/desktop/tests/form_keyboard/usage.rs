//! The Usage dashboard (`C2pTYF`, `p4eVV6`): the sidebar item and the status bar's *Tokens today*
//! open it; a finished run shows in the summary, the chart, its table and *Most expensive*; a
//! project without runs says so and offers the last seven days; plan limits without reports say
//! so; the warning threshold saves for every project; All projects shows every project's usage.
//! `cargo test -p orchester-desktop --test form_keyboard usage`.

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{create_and_run, init_repository, latest_run, programs, seed},
};

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

fn shows_text(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

#[test]
fn usage_dashboard_shows_a_finished_run_from_the_sidebar_and_the_status_bar() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  desktop.wait_until("the run to finish", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().is_terminal())
  });
  desktop.press("nav-usage");
  desktop.wait_until("the run in the summary", |desktop| {
    shows_text(desktop, "usage-summary-runs", "1 finished")
  });
  assert!(shows(&mut desktop, "usage-readback", "compared with"));
  desktop.wait_for("usage-tokens-chart");
  assert!(
    shows(&mut desktop, "usage-expensive", "KON-1"),
    "the task is the most expensive"
  );
  desktop.press("usage-tokens-view-1");
  desktop.wait_for("usage-tokens-table");
  // Below the fold: clicked where it is, as the page would be scrolled to it.
  desktop.click("usage-expensive-mode-1");
  desktop.wait_for("usage-expensive-runs");
  assert!(shows(&mut desktop, "usage-expensive-runs", "KON-1 · Run 1"));

  desktop.press("nav-overview");
  desktop.wait_for("status-tokens");
  desktop.press("status-tokens");
  desktop.wait_for("usage-summary");
}

#[test]
fn usage_without_runs_says_so_offers_seven_days_and_no_plan_limits() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop.wait_until("the empty week", |desktop| {
    shows_text(desktop, "usage-empty", "No runs in Kontur in this period")
  });
  assert!(shows(&mut desktop, "usage-limits", "No plan limits recorded yet"));
  assert!(
    shows(&mut desktop, "usage-summary-cache", "—"),
    "a share without input is a dash"
  );
  desktop.press("usage-period-0");
  desktop.wait_until("the empty day", |desktop| {
    shows_text(desktop, "usage-empty", "No runs in Kontur today")
  });
  desktop.press("usage-show-week");
  desktop.wait_until("the week again", |desktop| {
    shows_text(desktop, "usage-empty", "in this period") && !has(desktop, "usage-show-week")
  });
}

fn set_threshold(desktop: &mut Desktop, text: &str) {
  desktop
    .tree
    .get_element_by_id_mut("usage-threshold")
    .and_then(|element| element.as_text_input())
    .expect("the threshold field")
    .set_value(text);
  desktop.pass();
}

#[test]
fn usage_warning_threshold_saves_for_every_project() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop.wait_for("usage-limits-menu");
  desktop.press("usage-limits-menu");
  desktop.wait_for("usage-threshold");
  set_threshold(&mut desktop, "101");
  assert!(shows(
    &mut desktop,
    "usage-threshold-popover",
    "Enter a whole number from 1 to 100."
  ));
  set_threshold(&mut desktop, "70");
  desktop.wait_until("the threshold saved", |desktop| {
    shows_text(desktop, "usage-threshold-popover", "Saved for all projects")
  });
}

#[test]
fn usage_tab_of_all_projects_reads_every_project() {
  let mut desktop = Desktop::with_other_project(|_| {}, |_| {});
  desktop.wait_for("project-selector");
  desktop.press("nav-all-projects");
  desktop.wait_for("all-projects-tab-usage");
  desktop.press("all-projects-tab-usage");
  desktop.wait_until("both projects read", |desktop| {
    shows_text(desktop, "usage-readback", "2 projects")
  });
  assert!(
    has(&mut desktop, "usage-filter-project"),
    "Project comes first among the filters"
  );
  desktop.press("all-projects-tab-projects");
  desktop.wait_for("project-row-1");
}
