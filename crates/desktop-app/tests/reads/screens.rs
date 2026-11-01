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
  let line = agents.settle();
  assert_eq!(
    line.get("agents"),
    None,
    "a transcript line reads no profiles: {line:?}"
  );
  agents.report(true);
  let state = agents.settle();
  assert_eq!(
    state.get("agents"),
    Some(&1),
    "a run's state reads the profiles' uses: {state:?}"
  );
}

#[test]
fn reads_the_code_index_again_on_its_own_changes_not_on_lines() {
  // No run runs.
  let mut index = Counted::open("empty");
  index.show(ProjectPage::Settings(Default::default()), "screen-project_settings");
  index.window.press("settings-tab-context");
  index.settle();
  index.report(false);
  assert_eq!(index.settle(), counts(&[]), "a transcript line reads nothing");
  let services = index.window.launch.services().expect("the session started").clone();
  services.changes.record(index.seeded.project, Outside::CodeIndex);
  let changed = index.settle();
  assert_eq!(changed.get("code index"), Some(&1), "{changed:?}");
  index.report(true);
  let state = index.settle();
  assert_eq!(
    state.get("code index"),
    Some(&1),
    "a run's state reads what uses an index: {state:?}"
  );
}

#[test]
fn reads_skills_again_when_a_run_changes_state_not_when_tasks_are_saved() {
  let mut skills = Counted::open("tasks");
  skills.show(ProjectPage::Skills(Default::default()), "screen-skills");
  draft(&skills.root());
  skills.window.projects().feed(skills.seeded.project).saved();
  let saved = skills.settle();
  assert_eq!(saved.get("skills"), None, "a task saved reads no skills: {saved:?}");
  assert_eq!(saved.get("project tasks"), Some(&1), "the tasks are read: {saved:?}");
  skills.report(true);
  let state = skills.settle();
  assert_eq!(
    state.get("skills"),
    Some(&1),
    "a run's state reads the skills' uses: {state:?}"
  );
}

/// Waits until the rows of `projects` show what their data says: their reading ended, not only started.
fn read_rows(all: &mut Counted, projects: &[ProjectId]) {
  for project in projects {
    let active = format!("project-row-active-{project}");
    all.window.until("the row's data", |window| {
      window
        .texts(&active)
        .is_some_and(|texts| texts.concat().parse::<u64>().is_ok())
    });
  }
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
  read_rows(&mut all, &[other.project.id()]);
  let first = all.settle();
  assert_eq!(first.get("listed project"), Some(&2), "both are read first: {first:?}");
  let before = all.window.text(&status);
  let services = all.window.launch.services().expect("the session started").clone();
  // Its runner program does not exist: the runner reports it cannot start.
  services.runners.ensure(&other.repo_root, "Other");
  let report = all.settle();
  assert_eq!(report, counts(&[("listed project", 1)]), "only Other is read again");
  assert_ne!(
    all.window.text(&status),
    before,
    "Other's row shows what its runner reported"
  );
}

#[test]
fn reads_no_listed_project_again_when_the_list_changes_after_a_report() {
  let mut all = Counted::open("tasks");
  let workspace = all.window.launch.workspace().expect("the session started").clone();
  let other = add_project(
    &workspace,
    (ProjectId::generate(), "OTH", "Other"),
    &all.window.data.repo("repo-other"),
  );
  let third = add_project(
    &workspace,
    (ProjectId::generate(), "THR", "Third"),
    &all.window.data.repo("repo-third"),
  );
  all.window.navigation().go(Route::AllProjects);
  let (row, remove) = (
    format!("project-row-{}", third.project.id()),
    format!("project-remove-{}", third.project.id()),
  );
  read_rows(&mut all, &[other.project.id(), third.project.id()]);
  all.settle();
  let services = all.window.launch.services().expect("the session started").clone();
  services.runners.ensure(&other.repo_root, "Other");
  assert_eq!(all.settle(), counts(&[("listed project", 1)]), "Other is read again");
  // Removing Third lists the projects again, and follows their runners anew.
  all.window.hover(&row);
  all.window.press(&remove);
  all.window.press("remove-project-confirm");
  all
    .window
    .until("Third removed", |window| window.tree.get_element_by_id(&row).is_none());
  let removed = all.settle();
  assert_eq!(
    removed.get("listed project"),
    None,
    "what was reported before is not reported again: {removed:?}"
  );
}

#[test]
fn reads_the_code_index_on_a_line_while_a_run_runs() {
  // KON-12 runs: its runner may start building an index the tab does not know of yet.
  let mut index = Counted::open("conversation");
  index.show(ProjectPage::Settings(Default::default()), "screen-project_settings");
  index.window.press("settings-tab-context");
  index.settle();
  index.report(false);
  let line = index.settle();
  assert_eq!(line.get("code index"), Some(&1), "{line:?}");
}
