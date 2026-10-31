//! The run's *Browser* line on its Summary (`10 · Run · Browser line` `RGeKO`) on the tabs its project's browser
//! reports, as a runner would: none while the run has no tab, how many it has and their hosts, and *Show*, which opens
//! the Browser window on the project. Which tabs it counts and the tab *Show* asks for are the model's
//! (`tests/browser_window/tabs.rs`).

use std::path::Path;

use orchester_core::{
  id::RunId,
  store::{RunStore, TaskStore},
};
use orchester_ipc::browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, TabOwner};
use orchester_store::ProjectDb;

use crate::page::Page;

fn tab(id: &str, run: RunId, url: &str) -> BrowserTab {
  BrowserTab {
    id: BrowserTabId::new(id),
    owner: TabOwner::Run { run },
    title: format!("Page {id}"),
    url: url.to_owned(),
    loading: false,
  }
}

/// The latest run of the task `key` of the project at `root`.
fn latest_run(root: &Path, key: &str) -> RunId {
  let db = ProjectDb::open(root).expect("the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  db.runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .map(|run| run.expect("the seeded run reads"))
    .max_by_key(|run| run.number())
    .expect("a run")
    .id()
}

#[test]
fn summary_browser_line_counts_the_runs_tabs_and_shows_the_window() {
  let mut page = Page::seeded("task-states");
  let key = page.open("failed");
  let root = page.window.data.dir.path().join("repo-task-states");
  let run = latest_run(&root, &key);
  assert!(!page.has("task-browser"), "a run without tabs has no line");
  let services = page.window.launch.services().expect("the session started").clone();
  let browser = services.runners.browser_window();
  let feed = |tabs: Vec<BrowserTab>| {
    browser.receive_for_test(
      &root,
      "Kontur",
      BrowserEvent::Changed(BrowserState {
        tabs,
        shown: None,
        mode: BrowserMode::Headless,
      }),
    );
  };
  let other = RunId::generate();
  feed(vec![
    tab("O", other, "http://localhost:3000/"),
    tab("A", run, "http://localhost:5173/endpoints"),
  ]);
  page
    .window
    .until("the line", |window| window.texts("task-browser").is_some());
  assert_eq!(page.text("task-browser-title"), "Browser");
  assert_eq!(
    page.text("task-browser-tabs"),
    "1 tab · localhost:5173",
    "another run's tab is not this run's"
  );
  feed(vec![
    tab("O", other, "http://localhost:3000/"),
    tab("A", run, "http://localhost:5173/endpoints"),
    tab("D", run, "https://docs.rs/reqwest"),
    tab("B", run, "http://localhost:5173/settings"),
  ]);
  page.window.until("the line with three tabs", |window| {
    window
      .texts("task-browser-tabs")
      .is_some_and(|texts| texts.join(" ") == "3 tabs · localhost:5173, docs.rs")
  });
  assert_eq!(browser.shown_project(), None, "the window is closed");
  page.click("task-browser-show");
  page.window.until("the window on the run's project", |_| {
    browser.shown_project().as_deref() == Some(root.as_path())
  });
}
