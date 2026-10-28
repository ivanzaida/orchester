//! Where the Overview's links lead: a wait's link to where it is decided, as the Needs you panel's does (its task's
//! page, pointing at its decision card, or the project's Memory page), *Show all* and *Done* into Tasks' views, the
//! merge queue's *Review* to Review › Merge queue, *Accounts* to Settings › Accounts, *Delivery* to the Delivery view,
//! and the header's Import resources and New task. A request whose task cannot be read has no link: it says why.

use orchester_app::routes::{
  AccountsView, ImportTab, MemoryTab, OverviewTab, ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView,
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  delivery::DeliveryPeriod,
  tasks::attention::{self, NeedKind},
};

use crate::harness::Seeded;

fn task(seeded: &Seeded, key: &str, tab: TaskTab) -> Route {
  let key: TaskKey = key.parse().expect("a task key");
  Route::Project(seeded.project, ProjectPage::Task(key, TaskView::of(tab)))
}

#[test]
fn overview_a_waits_link_opens_where_it_is_decided() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let items = attention::items(&seeded.tasks());
  let first = items.first().expect("something waits for the person");
  let tab = match first.kind {
    NeedKind::Approval | NeedKind::Question | NeedKind::Input => TaskTab::Conversation,
    _ => TaskTab::Summary,
  };
  let link = seeded
    .ids("overview-wait-0-")
    .into_iter()
    .find(|id| id.ends_with("-open"))
    .expect("the first wait's link");
  seeded.click(&link);
  let key = &first.task().expect("the first wait's task").key;
  assert_eq!(seeded.window.navigation().route_now(), task(&seeded, key, tab));
}

/// The element `id` is ringed as the card a navigation pointed at.
fn pointed(seeded: &mut Seeded, id: &str) -> bool {
  seeded
    .window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class("pointed"))
}

#[test]
fn overview_a_plan_and_a_limit_open_at_their_card_ringed_as_from_the_panel() {
  for (kind, card, words) in [
    ("plan", "plan-review", "Review plan ↗"),
    ("limit", "raise-limit", "Raise limit ↗"),
  ] {
    let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
    let line = seeded
      .ids("overview-wait-")
      .into_iter()
      .find(|id| id.ends_with(&format!("-{kind}")))
      .unwrap_or_else(|| panic!("a {kind} waits in the seed"));
    assert_eq!(seeded.text(&format!("{line}-open")), words);
    let key = line
      .trim_start_matches("overview-wait-")
      .split_once('-')
      .and_then(|(_, rest)| rest.strip_suffix(&format!("-{kind}")))
      .expect("the line names its task")
      .to_owned();
    seeded.click(&format!("{line}-open"));
    assert_eq!(
      seeded.window.navigation().route_now(),
      task(&seeded, &key, TaskTab::Summary)
    );
    seeded.until(&format!("the {kind}'s card ringed"), |seeded| pointed(seeded, card));
  }
}

#[test]
fn overview_memory_records_open_the_project_memory_page() {
  let mut seeded = Seeded::overview("tasks-memory", OverviewTab::Running);
  let line = seeded
    .ids("overview-wait-")
    .into_iter()
    .find(|id| id.ends_with("-unknown-memory"))
    .expect("the records wait as one line");
  assert_eq!(seeded.text(&format!("{line}-open")), "Review records ↗");
  seeded.click(&format!("{line}-open"));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Memory(Some(MemoryTab::Waiting)))
  );
}

#[test]
fn overview_a_request_whose_run_cannot_be_read_says_why_in_place_of_its_link() {
  let mut seeded = Seeded::damaging_approval_run("task-states");
  let line = seeded
    .ids("overview-wait-")
    .into_iter()
    .find(|id| id.ends_with("-unknown-approval"))
    .expect("the request without its task");
  assert!(!seeded.shows(&format!("{line}-open")), "nothing to open");
  assert_eq!(
    seeded.text(&format!("{line}-reason")),
    "Its task can't be read, so there is nothing to open."
  );
}

#[test]
fn overview_side_links_lead_to_their_places() {
  let mut seeded = Seeded::overview("delivery", OverviewTab::Running);
  let project = seeded.project;
  for (link, place) in [
    (
      "overview-merge-queue-review",
      Route::Project(project, ProjectPage::MergeQueue),
    ),
    (
      "overview-landed-done",
      Route::Project(project, ProjectPage::Tasks(TaskList::new(TaskListTab::Done))),
    ),
    (
      "overview-delivery-open",
      Route::Project(project, ProjectPage::Delivery(DeliveryPeriod::default())),
    ),
  ] {
    seeded.go(OverviewTab::Running);
    seeded.until(link, |seeded| seeded.shows(link));
    seeded.click(link);
    assert_eq!(seeded.window.navigation().route_now(), place, "{link}");
  }
}

#[test]
fn overview_accounts_link_opens_settings_accounts() {
  let mut seeded = Seeded::overview("usage", OverviewTab::Running);
  if !seeded.shows("overview-limits-accounts") {
    // No account of the seed is near a limit: Plan limits stays off the page.
    assert!(!seeded.shows("overview-limits"));
    return;
  }
  seeded.click("overview-limits-accounts");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Accounts(AccountsView::default())
  );
}

#[test]
fn overview_header_actions_lead_to_their_screens() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.click("overview-import");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::Import(ImportTab::All)),
    "Import resources opens on All"
  );
  seeded.go(OverviewTab::Running);
  seeded.until("the header", |seeded| seeded.shows("overview-new-task"));
  seeded.click("overview-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewTask)
  );
}
