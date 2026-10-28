//! What the Overview shows in each of its states (`Overview · redesign` `iRvT6`): the busy page's sections counted as
//! `orchester_workspace::{tasks::attention, overview}` counts them, the quiet page, a new project's getting-started
//! list, the tasks being read and a reading that failed; and the header, with Open Orchestrator, in every one.

use std::{thread, time::Duration};

use orchester_app::routes::{OrchestratorChat, OverviewTab, ProjectPage, Route};
use orchester_core::run::InterruptCause;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  overview::{at_work::at_work, landed::landed, queue::queue, start::steps},
  tasks::{attention, checkout::Checkout, ready_to_run::ProjectReadiness},
};

use crate::harness::Seeded;

#[test]
fn overview_digest_lists_what_waits_oldest_first_with_its_kind() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let items = attention::items(&seeded.tasks());
  assert!(!items.is_empty(), "the seed has things waiting for the person");
  let title = seeded.text("overview-waits-title");
  assert!(title.starts_with(&format!("{} ", items.len())), "{title}");
  let lines: Vec<String> = seeded
    .ids("overview-wait-")
    .into_iter()
    .filter(|id| id.matches('-').count() >= 4 && !id.ends_with("-open") && !id.ends_with("-what"))
    .filter(|id| !id.ends_with("-age") && !id.ends_with("-reason"))
    .collect();
  assert_eq!(lines.len(), items.len().min(6), "{lines:?}");
  for (index, (line, item)) in lines.iter().zip(&items).enumerate() {
    let key = item.task().map_or("unknown", |task| &task.key);
    assert!(
      line.starts_with(&format!("overview-wait-{index}-{key}-")),
      "{line} is item {index}"
    );
  }
  for pair in items.windows(2) {
    assert!(pair[0].since() <= pair[1].since(), "oldest first");
  }
}

#[test]
fn overview_ready_to_merge_waits_only_while_the_merge_waits_for_the_person() {
  // A run whose pipeline the merge queue delivers waits in the digest only at its approval.
  let mut seeded = Seeded::overview("delivery", OverviewTab::Running);
  let tasks = seeded.tasks();
  let merges: Vec<String> = seeded
    .ids("overview-wait-")
    .into_iter()
    .filter(|id| id.ends_with("-merge"))
    .collect();
  assert_eq!(merges.len(), 1, "only the run waiting for its approval: {merges:?}");
  assert!(seeded.text("overview-merge-queue-state").contains("waits for you"));
  assert_eq!(
    attention::items(&tasks)
      .iter()
      .filter(|item| item.kind == attention::NeedKind::ReadyToMerge)
      .count(),
    1
  );
  // While the queue delivers by itself the runs are on Landed's line, not waits.
  let mut blocked = Seeded::overview("delivery-blocked", OverviewTab::Running);
  assert!(
    !blocked.ids("overview-wait-").iter().any(|id| id.ends_with("-merge")),
    "no merge waits for the person while the queue delivers"
  );
  assert_eq!(
    blocked.text("overview-merge-queue-state"),
    "blocked: your checkout keeps delivery out · 1 waits"
  );
}

#[test]
fn overview_landed_counts_merges_only() {
  let mut seeded = Seeded::overview("delivery", OverviewTab::Running);
  let tasks = seeded.tasks();
  let merges = landed(&tasks, orchester_core::time::Timestamp::now().local_day_start());
  assert_eq!(merges.today.len(), 1, "the seed merged one run today");
  assert_eq!(seeded.text("overview-landed-today"), "Today · 1 merged");
  let lines = seeded.ids("overview-landed-KON-");
  assert!(
    lines.iter().all(|id| merges
      .today
      .iter()
      .any(|run| id.starts_with(&format!("overview-landed-{}", run.key)))),
    "a run that passed every step but is not merged is no line of Landed: {lines:?}"
  );
}

/// Stores the Codex connection the seed's profiles name, which the seed leaves missing so nothing can run.
fn store_the_profiles_connection(repo: &std::path::Path) {
  use orchester_core::{
    connection::{AgentConnection, AgentProvider},
    name::Name,
    resource::{AgentProfile, ResourceKind},
    store::{ConnectionStore, ResourceStore},
    time::Timestamp,
  };
  let mut db = orchester_store::ProjectDb::open(repo).expect("the project database");
  let mut named = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile).expect("the profiles") {
    let version = db.latest_version::<AgentProfile>(resource.id()).expect("a profile");
    named.extend(version.map(|version| version.body().connection()));
  }
  named.dedup();
  for id in named {
    let name = Name::new("Codex · Kontur").expect("a name");
    let connection = AgentConnection::new(id, name, AgentProvider::CodexCli, repo.join("codex"), Timestamp::now())
      .expect("a connection");
    db.create_connection(&connection.into_connection()).expect("stored");
  }
}

#[test]
fn overview_at_work_names_each_runs_step_account_and_model() {
  let seeded = Seeded::prepared("tasks", false, store_the_profiles_connection);
  let mut seeded = Seeded::shown(seeded, OverviewTab::Running);
  let lines = at_work(&seeded.tasks());
  assert!(!lines.is_empty(), "the seed has runs under way");
  for line in &lines {
    let id = format!("overview-at-work-{}", line.key);
    assert!(seeded.shows(&id), "{id}");
    let agent = line
      .work
      .as_ref()
      .and_then(|work| work.agent.as_ref())
      .unwrap_or_else(|| panic!("an agent works on {}: {line:#?}", line.key));
    let words = seeded.text(&format!("{id}-agent-words"));
    // Its model is the profile's until a launch says otherwise; its account, once the session lists it, comes first.
    assert!(words.ends_with(&*agent.model), "{id}: {words}");
    assert_eq!(agent.provider, orchester_core::connection::AgentProvider::CodexCli);
    let number = line.step.as_ref().map_or(0, |step| step.number);
    let step = seeded.text(&format!("{id}-step"));
    assert!(step.starts_with(&format!("{number:02} ")), "{id}: {step}");
  }
}

#[test]
fn overview_queue_says_why_each_task_waits_in_start_order() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let lines = queue(&seeded.tasks(), false);
  let shown: Vec<String> = seeded
    .ids("overview-queue-KON-")
    .into_iter()
    .filter(|id| id.matches('-').count() == 3)
    .collect();
  let expected: Vec<String> = lines
    .iter()
    .take(5)
    .map(|line| format!("overview-queue-{}", line.key))
    .collect();
  assert_eq!(shown, expected, "start order");
  if let Some(first) = lines.first() {
    assert_eq!(seeded.text(&format!("overview-queue-{}-place", first.key)), "next");
    let why = seeded.text(&format!("overview-queue-{}-why", first.key));
    assert!(!why.is_empty() && !why.contains("overview."), "{why}");
  }
}

#[test]
fn overview_quiet_says_nothing_waits_and_lists_drafts_ready_to_start() {
  let mut seeded = Seeded::overview("overview-quiet", OverviewTab::Running);
  assert_eq!(
    seeded.text("overview-quiet-title"),
    "Nothing waits for you, and nothing is running."
  );
  assert!(!seeded.shows("overview-waits") && !seeded.shows("overview-at-work"));
  assert_eq!(seeded.text("overview-ready-count"), "2");
  assert_eq!(seeded.text("overview-landed-today"), "Today · 2 merged");
}

/// The states sheet's seed (05 `JzNRO`): lists longer than they show end in *Show all N*, and Plan limits shows an
/// account near its limit, one at it, and one whose window reset since its reading, greyed.
#[test]
fn overview_states_seed_reaches_long_lists_and_every_plan_limit_look() {
  let mut seeded = Seeded::overview("overview-states", OverviewTab::Running);
  let waits = attention::items(&seeded.tasks()).len();
  assert!(waits > 6, "{waits} wait");
  assert_eq!(
    seeded.text("overview-waits-all"),
    format!("Show all {waits} in Tasks ↗")
  );
  assert!(seeded.shows("overview-queue-all"), "more than five are queued");
  let values = |seeded: &mut Seeded| -> Vec<String> {
    let mut values: Vec<String> = seeded
      .ids("overview-limit-")
      .into_iter()
      .filter(|id| id.ends_with("-value"))
      .map(|id| seeded.text(&id))
      .collect();
    values.sort();
    values
  };
  seeded.until("the three accounts", |seeded| values(seeded).len() == 3);
  assert_eq!(
    values(&mut seeded),
    ["86% of 5-hour", "Reached · 5-hour", "unknown since reset"]
  );
}

#[test]
fn overview_of_a_new_project_marks_its_steps_done_from_its_state() {
  let mut seeded = Seeded::overview("empty", OverviewTab::Running);
  seeded.until("the steps", |seeded| seeded.shows("overview-start-task"));
  let readiness = ProjectReadiness::load_in(&seeded.repo).expect("the readiness reads");
  let lines = steps(&readiness, 0, ", ");
  for line in &lines {
    let state = if line.done { "done" } else { "todo" };
    let id = format!("overview-start-{}-{state}", line.step.name());
    assert!(seeded.shows(&id), "{id}");
  }
  let done = lines.iter().filter(|line| line.done).count();
  assert!(done > 0, "the seed publishes its agents");
  assert_eq!(seeded.text("overview-start-done"), format!("{done} of 4 done"));
  assert!(seeded.shows("overview-start-task-todo"), "no task yet");
}

#[test]
fn overview_offers_open_orchestrator_in_every_state() {
  for scenario in ["tasks", "overview-quiet", "empty"] {
    let mut seeded = Seeded::overview(scenario, OverviewTab::Running);
    seeded.click("overview-open-orchestrator");
    assert_eq!(
      seeded.window.navigation().route_now(),
      Route::Project(seeded.project, ProjectPage::Orchestrator(OrchestratorChat::default())),
      "{scenario}"
    );
  }
  let mut failed = Seeded::reading("tasks-unreadable", false);
  failed.go(OverviewTab::Running);
  failed.until("the failure", |seeded| seeded.shows("overview-load-error"));
  assert!(
    failed.shows("overview-open-orchestrator"),
    "a page that can't be read keeps its header"
  );
}

#[test]
fn overview_while_the_tasks_are_read_shows_heads_and_bones_never_an_empty_page() {
  let mut seeded = Seeded::reading("tasks", true);
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  seeded.go(OverviewTab::Running);
  seeded.until("the page", |seeded| seeded.shows("screen-overview"));
  for _ in 0..20 {
    seeded.window.tree.tick_futures();
    seeded.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert!(seeded.shows("overview-open-orchestrator"), "the header is real");
  assert_eq!(seeded.text("overview-waits-title"), "Waiting for you");
  assert!(seeded.shows("overview-waits-bones") && seeded.shows("overview-at-work-bones"));
  assert!(!seeded.shows("overview-start") && !seeded.shows("overview-quiet"));
  hold.set(false);
  seeded.until("the tasks", |seeded| seeded.shows("overview-merge-queue"));
  assert!(!seeded.shows("overview-waits-bones"));
}

#[test]
fn overview_when_the_tasks_cannot_be_read_says_why_with_try_again() {
  let mut seeded = Seeded::reading("tasks-unreadable", false);
  seeded.go(OverviewTab::Running);
  seeded.until("the failure", |seeded| seeded.shows("overview-load-error"));
  assert_eq!(seeded.text("overview-load-error-title"), "The tasks couldn't be read");
  assert!(seeded.shows("overview-load-error-retry"));
  assert!(!seeded.shows("overview-quiet") && !seeded.shows("overview-start"));
}

#[test]
fn overview_keeps_a_task_whose_latest_run_cannot_be_read() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let rows = seeded.ids("overview-unreadable-");
  assert!(!rows.is_empty(), "the seed's unreadable run keeps its row");
}

#[test]
fn overview_keeps_the_recovery_notice_above_its_heading() {
  let mut seeded = Seeded::overview("interrupted", OverviewTab::Running);
  seeded.until("the recovery notice", |seeded| seeded.shows("recovery-notice"));
  assert!(seeded.top("recovery-notice") < seeded.top("overview-header"));
}

/// The compact notice says why the runs stopped (`xPTrL` in `JAIYi`), as the open list does.
#[test]
fn overview_recovery_notice_says_why_the_runs_stopped() {
  let mut seeded = Seeded::overview("interrupted", OverviewTab::Running);
  seeded.until("the recovery notice", |seeded| seeded.shows("recovery-review"));
  let services = seeded.window.launch.services().expect("the session started").clone();
  let runs = services.recovery.state().runs_of(seeded.project);
  let quit = runs.iter().all(|run| run.cause == InterruptCause::QuitNow);
  let why = if quit {
    "You chose Quit now."
  } else {
    "Orchester stopped unexpectedly."
  };
  let said = seeded.text("recovery-notice");
  assert!(
    said.contains(&format!("{why} Their worktrees and commits are kept.")),
    "{said}"
  );
}

/// The checkout beside the name says what git tells of it: the seed's repository has no network `origin`, so its
/// branch follows the key.
#[test]
fn overview_names_the_checkouts_branch_beside_the_name() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  assert_eq!(seeded.text("overview-title"), "Kontur");
  let checkout = Checkout::read(&Programs::processes(), &seeded.repo);
  assert_eq!(checkout.repository, None, "the seed has no network origin");
  let branch = checkout.branch.expect("the seed checks out a branch");
  let expected = format!("KON · {branch}");
  seeded.until("git's branch", |seeded| seeded.text("overview-checkout") == expected);
}
