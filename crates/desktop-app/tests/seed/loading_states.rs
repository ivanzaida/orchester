use orchester_core::store::TaskStore;
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::data::ProjectAgents,
  pull_requests::list::{PullRequestList, load_list_in},
};

use super::Seed;

#[test]
fn seed_loading_states_has_a_module_with_tasks_and_a_linked_pull_request_list() {
  let seed = Seed::of("loading-states");
  let db = ProjectDb::open(seed.repo()).expect("the project's database");
  let modules = db.modules().expect("modules");
  assert_eq!(modules.len(), 1);
  let tasks = db.tasks().expect("tasks");
  let in_module = tasks
    .iter()
    .filter(|task| task.module() == Some(modules[0].id()))
    .count();
  assert_eq!(in_module, 2);
  assert!(matches!(
    load_list_in(seed.repo()).expect("the stored pull requests"),
    PullRequestList::Linked(_)
  ));
  assert!(ProjectAgents::for_screen(seed.repo()).is_ok());
}

#[test]
fn seed_tasks_profiles_read_beside_a_damaged_running_run() {
  let seed = Seed::of("tasks");
  let listed = ProjectAgents::for_screen(seed.repo()).expect("the profiles read past KON-24's damaged run");
  assert!(!listed.profiles.is_empty());
}
