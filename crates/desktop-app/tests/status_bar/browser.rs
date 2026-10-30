//! The Browser item (`Browser 3 tabs` `PwuEd` in `RGeKO`): as its facts say, and in the real window on the tabs a
//! project's browser reports as a runner would, its press opening the Browser window on the project.

use orchester_core::time::Timestamp;
use orchester_ipc::browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, TabOwner};
use orchester_workspace::browser::BrowserWindow;

use super::{
  kontur::{self as seed, SeedRequest, scenarios},
  support::{Data, Window},
  *,
};

fn with_tabs(browser_tabs: usize) -> BarFacts {
  facts(Some(ProjectFacts {
    browser_tabs,
    ..kontur(runner(RunnerState::Online), ProjectTasks::default())
  }))
}

#[test]
fn status_browser_item_says_how_many_tabs_and_asks_for_the_window() {
  assert!(
    Bar::of(with_tabs(0)).texts("status-browser").is_none(),
    "no tabs, no item"
  );
  assert_eq!(
    Bar::of(with_tabs(1)).texts("status-browser"),
    Some(vec!["Browser 1 tab".to_owned()])
  );
  let mut bar = Bar::of(with_tabs(3));
  assert_eq!(bar.texts("status-browser"), Some(vec!["Browser 3 tabs".to_owned()]));
  bar.press("status-browser");
  assert_eq!(bar.asked(), [BarAction::ShowBrowser]);
}

#[test]
fn status_browser_item_stands_right_before_the_mcp_item() {
  let qa = McpStatus {
    kind: McpKind::Qa {
      folder: PathBuf::from(".tmp/qa/orc-194/mcp"),
      file: None,
    },
    settings: McpSettings::with_window_control(false),
    ..listening()
  };
  let mut bar = Bar::of(BarFacts {
    mcp: qa,
    ..with_tabs(3)
  });
  let (browser, mcp) = (bar.bounds("status-browser"), bar.bounds("status-mcp"));
  assert!(
    (mcp.x - (browser.x + browser.width)).abs() < 0.5,
    "{browser:?} ends where {mcp:?} starts"
  );
}

/// Without a project the bar names no project's browser.
#[test]
fn status_browser_item_stays_off_a_screen_outside_a_project() {
  assert!(Bar::of(facts(None)).texts("status-browser").is_none());
}

/// On a narrow bar its words give way and its globe stays, so the dot keeps its place.
#[test]
fn status_browser_item_keeps_its_globe_when_narrow() {
  let wide = Bar::of(with_tabs(3)).width("status-browser");
  let mut narrow = Bar::at(100, with_tabs(3));
  assert!(narrow.width("status-browser") > 0.0, "the globe stays");
  assert!(narrow.width("status-browser") < wide, "the words give way");
  assert!(narrow.width("status-health") > 0.0);
}

fn tab(id: &str) -> BrowserTab {
  BrowserTab {
    id: BrowserTabId::new(id),
    owner: TabOwner::Person,
    title: format!("Page {id}"),
    url: format!("http://localhost:5173/{id}"),
    loading: false,
  }
}

/// `browser`'s project at `root` reports the tabs `ids`, as its runner would.
fn report(browser: &BrowserWindow, root: &std::path::Path, ids: &[&str]) {
  browser.receive_for_test(
    root,
    "Kontur",
    BrowserEvent::Changed(BrowserState {
      tabs: ids.iter().map(|id| tab(id)).collect(),
      shown: None,
      mode: BrowserMode::Headless,
    }),
  );
}

/// The window on the `tasks` scenario, and where its project lies.
fn window_on_tasks() -> (Window, PathBuf) {
  let data = Data::new();
  let repo = data.dir.path().join("repo-tasks");
  let request = SeedRequest {
    data_dir: data.dir.path().to_path_buf(),
    repo: repo.clone(),
    scenario: scenarios::named("tasks").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  seed::seed(&request).expect("the scenario seeds");
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.until("the project to open", |window| {
    let project = window.navigation().route_now().project();
    project
      .and_then(|project| window.projects().state(project))
      .is_some_and(|state| state.repo_root().is_some())
  });
  (window, repo)
}

#[test]
fn status_browser_in_the_window_follows_the_tabs_of_the_shown_project_only() {
  let (mut window, repo) = window_on_tasks();
  let browser = window
    .launch
    .services()
    .expect("the session started")
    .runners
    .browser_window();
  assert!(window.texts("status-browser").is_none(), "hidden without tabs");
  report(&browser, std::path::Path::new("H:/projects/other"), &["O1", "O2"]);
  report(&browser, &repo, &[]);
  window.pass();
  assert!(
    window.texts("status-browser").is_none(),
    "another project's tabs are not this project's"
  );
  for (ids, words) in [
    (&["A"][..], "Browser 1 tab"),
    (&["A", "B", "C"][..], "Browser 3 tabs"),
    (&["A", "C"][..], "Browser 2 tabs"),
  ] {
    report(&browser, &repo, ids);
    window.until(words, |window| {
      window
        .texts("status-browser")
        .is_some_and(|texts| texts.iter().any(|text| text == words))
    });
  }
  report(&browser, &repo, &[]);
  window.until("the item to go with the tabs", |window| {
    window.texts("status-browser").is_none()
  });
}

#[test]
fn status_browser_in_the_window_goes_when_the_runner_ends() {
  let (mut window, repo) = window_on_tasks();
  let browser = window
    .launch
    .services()
    .expect("the session started")
    .runners
    .browser_window();
  report(&browser, &repo, &["A"]);
  window.until("the item", |window| window.texts("status-browser").is_some());
  browser.ended_for_test(&repo);
  window.until("the item to go with the browser", |window| {
    window.texts("status-browser").is_none()
  });
}

#[test]
fn status_browser_press_opens_the_window_on_the_project() {
  let (mut window, repo) = window_on_tasks();
  let browser = window
    .launch
    .services()
    .expect("the session started")
    .runners
    .browser_window();
  report(&browser, &repo, &["A", "B"]);
  window.until("the item", |window| window.texts("status-browser").is_some());
  assert_eq!(browser.shown_project(), None, "closed until asked for");
  window.press("status-browser");
  window.until("the window on the project", |_| {
    browser.shown_project().as_deref() == Some(repo.as_path())
  });
}
