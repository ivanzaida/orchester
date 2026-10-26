//! Needs you across projects, with two projects that wait: Kontur (the `tasks` scenario) and Atlas (`cards`). Each
//! tab of a project that waits shows a ✋ and the count of its project's Needs you, which follows the project; the
//! panel lists both projects whichever tab is active, the active tab's first; Review on another project's item opens
//! it in that project's tab, or a new one, and leaves the current tab where it was; Back returns to the tab it came
//! from; an item of the current tab's project stays in the tab; Ctrl+click opens a new tab.

use lurq::app::events::MouseButton;
use orchester_app::{
  navigation::{
    Navigation,
    press::{LinkPress, OpenIn},
    tabs::TabId,
  },
  project::TasksState,
  routes::{OverviewTab, ProjectPage, Route},
  ui::chat::links::{Followed, follow},
};
use orchester_core::time::Timestamp;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::tasks::board::BoardTab;

use super::{
  Seeded, Window, click, item_of, kontur, open, propose_memory, scenarios, seeded_with, summary_of,
  support::link_press::{Presser, press_control},
};

/// The plan that waits in Kontur, which opens its task's Summary.
const PLAN: &str = "Plan the GitHub app permissions";

struct Two {
  window: Window,
  kontur: Seeded,
  atlas: Seeded,
}

/// The window on Kontur's Overview, with Atlas in the list too, both waiting for the person.
fn two_projects() -> Two {
  let mut atlas = None;
  let (window, kontur) = seeded_with("tasks", |kontur| atlas = Some(seed_atlas(kontur)));
  Two {
    window,
    kontur,
    atlas: atlas.expect("Atlas is seeded"),
  }
}

/// Atlas: the `cards` scenario in a repository of its own, renamed and registered beside Kontur, used before it so
/// the session still opens on Kontur.
fn seed_atlas(kontur: &Seeded) -> Seeded {
  let folder = kontur.repo.parent().expect("the repository lies in the data folder");
  let request = kontur::SeedRequest {
    data_dir: folder.join("atlas-data"),
    repo: folder.join("repo-atlas"),
    scenario: scenarios::named("cards").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let atlas = kontur::seed(&request).expect("Atlas seeds");
  let renamed = rusqlite::Connection::open(atlas.repo.join(".orchester").join("orchester.db"))
    .expect("Atlas's database")
    .execute("UPDATE project SET name = 'Atlas'", [])
    .expect("Atlas is renamed");
  assert_eq!(renamed, 1);
  let project = ProjectDb::open(&atlas.repo).expect("Atlas opens").project().clone();
  let mut db = AppDb::open(&kontur.app_database).expect("the application database");
  let owner = db.cloud_account().expect("the account").map(|account| account.account);
  let earlier = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 3_600_000);
  db.register_project(&project, &atlas.repo, earlier, owner.as_ref())
    .expect("Atlas is registered");
  atlas
}

fn overview(seeded: &Seeded) -> Route {
  Route::Project(seeded.project, ProjectPage::Overview(OverviewTab::default()))
}

/// The tab `index` from the left.
fn tab(navigation: &Navigation, index: usize) -> TabId {
  navigation.tabs_now().tabs()[index].id()
}

/// What the ✋ of the tab `id` counts; `None` while it shows none.
fn badge(window: &mut Window, id: TabId) -> Option<String> {
  window
    .texts(&format!("tab-{id}-attention-count"))
    .map(|texts| texts.join(" "))
}

/// The count of the project's Needs you, of its tasks as they stand with its runner.
fn needs_you(window: &Window, seeded: &Seeded) -> usize {
  match window.projects().feed(seeded.project).tasks_with_runner(&seeded.repo) {
    TasksState::Ready(tasks) => BoardTab::NeedsYou.count(&tasks),
    state => panic!("the tasks are read: {state:?}"),
  }
}

/// Kontur's tab, then Atlas's at its Overview, active; once both badges show.
fn side_by_side(two: &mut Two) -> (TabId, TabId) {
  let navigation = two.window.navigation();
  navigation.open(overview(&two.atlas), OpenIn::NewTab);
  let (kontur, atlas) = (tab(&navigation, 0), tab(&navigation, 1));
  two.window.until("both tabs' counts", |window| {
    badge(window, kontur).is_some() && badge(window, atlas).is_some()
  });
  (kontur, atlas)
}

fn shown(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

#[test]
fn needs_you_tabs_show_and_follow_each_projects_count() {
  let mut two = two_projects();
  let (kontur, atlas) = side_by_side(&mut two);
  assert_eq!(badge(&mut two.window, kontur).as_deref(), Some("6"));
  let waiting = needs_you(&two.window, &two.atlas);
  assert!(waiting > 0, "Atlas waits");
  assert_eq!(
    badge(&mut two.window, atlas),
    Some(waiting.to_string()),
    "the active tab too"
  );
  propose_memory(&two.atlas);
  two.window.projects().feed(two.atlas.project).saved();
  let more = (waiting + 1).to_string();
  two.window.until("Atlas's count to follow", |window| {
    badge(window, atlas).as_deref() == Some(more.as_str())
  });
  assert_eq!(badge(&mut two.window, kontur).as_deref(), Some("6"), "Kontur's stays");
}

#[test]
fn needs_you_panel_keeps_every_project_the_active_tabs_first() {
  let mut two = two_projects();
  let (kontur, _) = side_by_side(&mut two);
  let (k, a) = (two.kontur.project, two.atlas.project);
  open(&mut two.window);
  let window = &mut two.window;
  assert!(
    shown(window, &format!("needs-you-group-{a}")),
    "Atlas's tab is active: Atlas first"
  );
  assert!(
    shown(window, &format!("needs-you-list-group-{k}")),
    "and Kontur after it"
  );
  click(window, "needs-you-close");
  window.navigation().select(kontur);
  window.pass();
  open(window);
  assert!(shown(window, &format!("needs-you-group-{k}")), "Kontur first now");
  assert!(shown(window, &format!("needs-you-list-group-{a}")), "and Atlas stays");
}

#[test]
fn needs_you_review_of_another_projects_item_activates_its_tab_and_back_returns() {
  let mut two = two_projects();
  let (kontur, atlas) = side_by_side(&mut two);
  open(&mut two.window);
  let plan = item_of(&mut two.window, PLAN);
  click(&mut two.window, &format!("{plan}-action"));
  let navigation = two.window.navigation();
  let tabs = navigation.tabs_now();
  assert_eq!(tabs.tabs().len(), 2, "no new tab");
  assert_eq!(tabs.active().id(), kontur, "Kontur's tab");
  assert_eq!(*tabs.active().route(), summary_of(&two.kontur, PLAN));
  assert_eq!(
    *tabs.tabs()[1].route(),
    overview(&two.atlas),
    "Atlas's tab stays where it was"
  );
  two
    .window
    .until("the panel to close", |window| window.texts("needs-you-panel").is_none());
  two.window.tree.mouse_up(700.0, 400.0, MouseButton::Other(4));
  two.window.pass();
  let tabs = navigation.tabs_now();
  assert_eq!(
    tabs.active().id(),
    atlas,
    "the back button returns to the tab it came from"
  );
  assert_eq!(*tabs.active().route(), overview(&two.atlas));
}

/// A project no tab shows is not in Needs you, so a chat's link (as a notification's place) is what opens one of
/// its items from another project's tab.
#[test]
fn a_link_to_another_projects_item_without_a_tab_opens_a_tab_of_its_own() {
  let mut two = two_projects();
  let navigation = two.window.navigation();
  navigation.go(overview(&two.atlas));
  let atlas = tab(&navigation, 0);
  two
    .window
    .until("Atlas's count", |window| badge(window, atlas).is_some());
  let plan = summary_of(&two.kontur, PLAN);
  let followed = follow(&navigation, &format!("orchester:{}", plan.path()), LinkPress::PLAIN);
  assert!(matches!(followed, Followed::Went(OpenIn::ActiveTab)), "{followed:?}");
  two.window.pass();
  let tabs = navigation.tabs_now();
  assert_eq!(tabs.tabs().len(), 2, "a new tab");
  assert_eq!(tabs.tabs()[0].id(), atlas);
  assert_eq!(
    *tabs.tabs()[0].route(),
    overview(&two.atlas),
    "the current tab stays where it was"
  );
  assert_eq!(
    tabs.active().id(),
    tab(&navigation, 1),
    "the new tab, right of it, is active"
  );
  assert_eq!(*tabs.active().route(), plan);
  navigation.back();
  two.window.pass();
  assert_eq!(
    navigation.tabs_now().active().id(),
    atlas,
    "Back returns to Atlas's tab"
  );
}

#[test]
fn needs_you_review_of_the_current_tabs_project_stays_in_the_tab() {
  let mut two = two_projects();
  let navigation = two.window.navigation();
  let kontur = tab(&navigation, 0);
  open(&mut two.window);
  let plan = item_of(&mut two.window, PLAN);
  click(&mut two.window, &format!("{plan}-action"));
  let tabs = navigation.tabs_now();
  assert_eq!((tabs.tabs().len(), tabs.active().id()), (1, kontur));
  assert_eq!(*tabs.active().route(), summary_of(&two.kontur, PLAN));
  navigation.back();
  two.window.pass();
  assert_eq!(
    navigation.route_now(),
    overview(&two.kontur),
    "Back walks the tab's history"
  );
}

#[test]
fn needs_you_ctrl_click_on_an_item_opens_a_new_tab() {
  let mut two = two_projects();
  let (kontur, atlas) = side_by_side(&mut two);
  open(&mut two.window);
  let plan = item_of(&mut two.window, PLAN);
  let action = format!("{plan}-action");
  if let Some(mut element) = two.window.tree.get_element_by_id_mut(&action) {
    element.scroll_into_view();
  }
  two.window.pass();
  press_control(
    &mut two.window,
    &action,
    (Presser::Person, MouseButton::Left),
    (true, false),
  );
  let tabs = two.window.navigation().tabs_now();
  let order: Vec<TabId> = tabs.tabs().iter().map(|tab| tab.id()).collect();
  assert_eq!(order.len(), 3, "a new tab");
  assert_eq!((order[0], order[1]), (kontur, atlas));
  assert_eq!(tabs.active().id(), atlas, "the current tab stays active");
  assert_eq!(*tabs.tabs()[2].route(), summary_of(&two.kontur, PLAN));
  assert_eq!(
    *tabs.tabs()[0].route(),
    overview(&two.kontur),
    "Kontur's tab is not taken"
  );
}
