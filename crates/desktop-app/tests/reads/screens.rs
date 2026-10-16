//! Screens that show a project's runs read again when a run changes state, not on a transcript's lines; Skills
//! not when tasks are saved.

use orchester_app::routes::ProjectPage;
use orchester_workspace::tasks::outside::Outside;

use super::{Counted, draft};

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
