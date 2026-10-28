//! The redesigned sidebar (`Sidebar · redesign` `WWXmX`) with a session's data: every route the old sidebar's
//! entries opened lands on its new place (`routes`), Configure folds per project (`configure`), Running shows only
//! while runs go (`running`), the Review hub's tabs keep their own routes (`review`), the switcher's menu holds
//! Project settings, All projects and Settings (`switcher`), and *Jump to* opens tasks, modules, pages, hub tabs and
//! actions (`jump`).

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed reports what it wrote; these tests read the window instead"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod configure;
mod jump;
mod review;
mod routes;
mod running;
mod switcher;

use orchester_app::routes::{OverviewTab, ProjectPage, Route};
use orchester_core::{id::ProjectId, time::Timestamp};
use support::{Data, Window};

fn overview(project: ProjectId) -> Route {
  Route::Project(project, ProjectPage::Overview(OverviewTab::default()))
}

/// A window on `scenario` of the QA seed, opened on its Kontur.
fn seeded(scenario: &str) -> (Window, ProjectId) {
  let data = Data::new();
  let seeded = kontur::seed(&kontur::SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: data.dir.path().join(format!("repo-{scenario}")),
    scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  })
  .expect("the scenario seeds");
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.navigation().go(overview(seeded.project));
  window.until("the project to open", |window| window.texts("nav-overview").is_some());
  (window, seeded.project)
}

/// Whether the sidebar's entry `id` shows as the selected one.
fn selected(window: &mut Window, id: &str) -> bool {
  window
    .tree
    .get_element_by_id(id)
    .is_some_and(|item| item.has_class("selected"))
}

/// The platform's command key (Ctrl, Cmd on macOS) with `key`, where the focus is.
fn command(window: &mut Window, (key, code): (&str, &str)) {
  let mac = cfg!(target_os = "macos");
  window
    .tree
    .key_down_with_meta(key.to_owned(), code.to_owned(), false, !mac, false, mac);
  window.tree.key_up(key.to_owned(), code.to_owned(), false, !mac, false);
  window.pass();
}

/// `key` pressed alone where the focus is.
fn key(window: &mut Window, (key, code): (&str, &str)) {
  window
    .tree
    .key_down(key.to_owned(), code.to_owned(), false, false, false);
  window.tree.key_up(key.to_owned(), code.to_owned(), false, false, false);
  window.pass();
}
