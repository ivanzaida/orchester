//! Screens that show a project's runs read again when a run changes state, not on a transcript's lines; Skills
//! not when tasks are saved; All projects only the project whose runner reported.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::outside::Outside;

use super::{Counted, draft, support::add_project};

/// What `pairs` count, as `Reads::take` gives it.
fn counts(pairs: &[(&str, usize)]) -> std::collections::BTreeMap<String, usize> {
  pairs.iter().map(|(read, count)| ((*read).to_owned(), *count)).collect()
}

#[test]
fn reads_agents_again_when_a_run_changes_state_not_on_its_lines() {
  let mut agents = Counted::open("agents");
  agents.show(ProjectPage::Agents, "screen-agents");
  agents.report(false);
  assert_eq!(agents.settle(), counts(&[]), "a transcript line reads nothing");
  agents.report(true);
  let state = agents.settle();
  assert_eq!(state.get("agents"), Some(&1), "a run's state reads the profiles' uses: {state:?}");
}

#[test]
fn reads_the_code_index_again_on_its_own_changes_not_on_lines() {
  let mut index = Counted::open("tasks");
  index.show(ProjectPage::Settings, "screen-project_settings");
  index.window.press("settings-tab-codeIndex");
  index.settle();
  index.report(false);
  assert_eq!(index.settle(), counts(&[]), "a transcript line reads nothing");
  let services = index.window.launch.services().expect("the session started").clone();
  services.changes.record(index.seeded.project, Outside::CodeIndex);
  let changed = index.settle();
  assert_eq!(changed.get("code index"), Some(&1), "{changed:?}");
  index.report(true);
  let state = index.settle();
  assert_eq!(state.get("code index"), Some(&1), "a run's state reads what uses an index: {state:?}");
}

#[test]
fn reads_skills_again_when_a_run_changes_state_not_when_tasks_are_saved() {
  let mut skills = Counted::open("tasks");
  skills.show(ProjectPage::Skills, "screen-skills");
  draft(&skills.root());
  skills.window.projects().feed(skills.seeded.project).saved();
  let saved = skills.settle();
  assert_eq!(saved.get("skills"), None, "a task saved reads no skills: {saved:?}");
  assert_eq!(saved.get("project tasks"), Some(&1), "the tasks are read: {saved:?}");
  skills.report(true);
  let state = skills.settle();
  assert_eq!(state.get("skills"), Some(&1), "a run's state reads the skills' uses: {state:?}");
}

#[test]
fn reads_only_the_listed_project_whose_runner_reported() {
  let mut all = Counted::open("tasks");
  let workspace = all.window.launch.workspace().expect("the session started").clone();
  let other = add_project(
    &workspace,
    (ProjectId::generate(), "OTH", "Other"),
    &all.window.data.repo("repo-other"),
  );
  all.window.navigation().go(Route::AllProjects);
  let status = format!("project-row-status-{}", other.project.id());
  all
    .window
    .until("both rows", |window| window.tree.get_element_by_id(&status).is_some());
  let first = all.settle();
  assert_eq!(first.get("listed project"), Some(&2), "both are read first: {first:?}");
  let before = all.window.text(&status);
  let services = all.window.launch.services().expect("the session started").clone();
  // Its runner program does not exist: the runner reports it cannot start.
  services.runners.ensure(&other.repo_root, "Other");
  let report = all.settle();
  assert_eq!(report, counts(&[("listed project", 1)]), "only Other is read again");
  assert_ne!(all.window.text(&status), before, "Other's row shows what its runner reported");
}
