//! The *Browser* window's side of the project's browser with a real browser: the state a subscribed desktop follows,
//! a frame of the tab shown, the person's click and typing, `<select>` lists and dialogs drawn by the window, the
//! person's own tabs apart from runs', frames dropped rather than queued, the report that an agent started browsing,
//! navigation, and the restart that reopens the tabs.

use std::time::Duration;

use orchester_core::id::RunId;
use orchester_ipc::{
  Event, Rejection,
  browser::{
    BrowserCommand, BrowserEvent, BrowserReply, BrowserState, BrowserTabId, DialogKind, KeyAction, Modifiers,
    MouseAction, MouseButton, Navigation, PersonInput, TabOwner, Viewport,
  },
};
use orchester_runner::browser::{BrowserError, ProjectBrowser, cdp::TargetId};
use serde_json::json;
use tokio::sync::mpsc;

use super::{PATIENCE, browser_installed, closed, observer, open_in_browser, pages};

/// The page of the form tests: a field, a button and a `<select>` at fixed places, in CSS pixels.
pub const FORM: &str = r#"<!doctype html><html><head><title>Page form</title><style>body{margin:0}</style></head><body>
<input id="field" style="position:fixed;left:0;top:0;width:300px;height:40px">
<button id="button" style="position:fixed;left:0;top:100px;width:300px;height:40px"
  onclick="this.textContent='clicked'">Click</button>
<select id="list" style="position:fixed;left:0;top:200px;width:300px;height:30px"
  onchange="document.title='chose '+this.value">
<option value="a">Alpha</option><option value="b">Beta</option><option value="c" disabled>Gamma</option>
<option value="d">Delta</option></select></body></html>"#;

/// A page that changes at every animation frame, so the screencast sends frames all the time.
pub const ANIMATED: &str = r#"<!doctype html><html><head><title>Page animated</title></head><body>
<div id="box" style="width:400px;height:400px"></div><script>let n=0;function step(){n++;
const box=document.getElementById('box');box.style.background='hsl('+(n%360)+',80%,50%)';box.textContent=n;
requestAnimationFrame(step)}requestAnimationFrame(step)</script></body></html>"#;

/// A page that asks with a prompt shortly after it loads and shows the answer in its title.
pub const DIALOG: &str = r#"<!doctype html><html><head><title>Page dialog</title></head><body><script>
setTimeout(()=>{document.title='answered '+prompt('How many?','3')},300)</script></body></html>"#;

/// The window as the desktop is: its events, as the server hands them over.
struct Desktop {
  sender: mpsc::UnboundedSender<Event>,
  events: mpsc::UnboundedReceiver<Event>,
  /// Every browser event read so far, in order.
  seen: Vec<BrowserEvent>,
}

impl Desktop {
  fn new() -> Self {
    let (sender, events) = mpsc::unbounded_channel();
    Self {
      sender,
      events,
      seen: Vec::new(),
    }
  }

  async fn serve(&self, browser: &ProjectBrowser, command: BrowserCommand) -> BrowserReply {
    browser
      .serve(command, &self.sender)
      .await
      .expect("the browser carries it out")
  }

  /// The next browser event `wanted` accepts, within the test's patience.
  async fn wait_for(&mut self, wanted: impl Fn(&BrowserEvent) -> bool) -> BrowserEvent {
    let found = tokio::time::timeout(PATIENCE, async {
      loop {
        if let Event::Browser(event) = self.events.recv().await.expect("the browser keeps sending") {
          self.seen.push(event.clone());
          if wanted(&event) {
            return event;
          }
        }
      }
    })
    .await;
    found.unwrap_or_else(|_| panic!("the event did not come in time; these did: {:#?}", self.seen))
  }

  /// The first state `wanted` accepts.
  async fn state(&mut self, wanted: impl Fn(&BrowserState) -> bool) -> BrowserState {
    match self
      .wait_for(|event| matches!(event, BrowserEvent::Changed(state) if wanted(state)))
      .await
    {
      BrowserEvent::Changed(state) => state,
      other => unreachable!("waited for a state, got {other:?}"),
    }
  }

  /// Every browser event that came so far.
  fn received(&mut self) -> &[BrowserEvent] {
    while let Ok(event) = self.events.try_recv() {
      if let Event::Browser(event) = event {
        self.seen.push(event);
      }
    }
    &self.seen
  }
}

fn viewport() -> Viewport {
  Viewport::new(800, 600, 100).expect("in bounds")
}

fn tab_of(target: &TargetId) -> BrowserTabId {
  BrowserTabId::new(target.as_str())
}

fn mouse(action: MouseAction, x: i32, y: i32) -> PersonInput {
  PersonInput::Mouse {
    action,
    x,
    y,
    button: MouseButton::Left,
    clicks: 1,
    modifiers: Modifiers::default(),
  }
}

async fn click(desktop: &Desktop, browser: &ProjectBrowser, tab: &BrowserTabId, (x, y): (i32, i32)) {
  for action in [MouseAction::Pressed, MouseAction::Released] {
    let input = mouse(action, x, y);
    desktop
      .serve(
        browser,
        BrowserCommand::Input {
          tab: tab.clone(),
          input,
        },
      )
      .await;
  }
}

async fn key(desktop: &Desktop, browser: &ProjectBrowser, tab: &BrowserTabId, name: &str) {
  for action in [KeyAction::Down, KeyAction::Up] {
    let input = PersonInput::Key {
      action,
      key: name.to_owned(),
      code: name.to_owned(),
      text: None,
      modifiers: Modifiers::default(),
    };
    desktop
      .serve(
        browser,
        BrowserCommand::Input {
          tab: tab.clone(),
          input,
        },
      )
      .await;
  }
}

/// What `expression` gives in `run`'s tab `tab`.
async fn evaluate(browser: &ProjectBrowser, run: RunId, tab: &TargetId, expression: &str) -> serde_json::Value {
  let answer = browser
    .call(
      run,
      tab,
      "Runtime.evaluate",
      json!({ "expression": expression, "returnByValue": true }),
    )
    .await
    .expect("the page is evaluated");
  answer["result"]["value"].clone()
}

async fn person_tab(desktop: &Desktop, browser: &ProjectBrowser, url: Option<String>) -> BrowserTabId {
  match desktop.serve(browser, BrowserCommand::OpenTab { url }).await {
    BrowserReply::TabOpened { tab } => tab,
    other => panic!("expected the person's new tab, got {other:?}"),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_window_gets_a_frame_of_the_tab_it_shows() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  assert_eq!(
    desktop.state(|_| true).await,
    BrowserState::default(),
    "a browser that does not run has no tabs"
  );

  let tab = person_tab(&desktop, &browser, Some(format!("{site}shown"))).await;
  let viewport = viewport();
  desktop
    .serve(
      &browser,
      BrowserCommand::Show {
        tab: tab.clone(),
        viewport,
      },
    )
    .await;
  let frame = tokio::time::timeout(PATIENCE, browser.next_frame())
    .await
    .expect("a frame in time");

  assert_eq!(frame.tab, tab);
  assert_eq!(frame.jpeg.get(..2), Some(&[0xFF, 0xD8][..]), "a JPEG");
  assert_eq!((frame.css_width, frame.css_height), (800, 600));
  let state = desktop
    .state(|state| state.shown.as_ref() == Some(&tab) && state.tabs.iter().any(|tab| tab.title == "Page shown"))
    .await;
  assert_eq!(state.tabs.len(), 1);
  assert_eq!(state.tabs[0].owner, TabOwner::Person);
  assert_eq!(state.tabs[0].url, format!("{site}shown"));

  desktop.serve(&browser, BrowserCommand::StopShowing).await;
  desktop.state(|state| state.shown.is_none()).await;
  browser.stop().await;
  desktop.state(|state| state.tabs.is_empty()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_person_click_and_typing_reach_the_page() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let desktop = Desktop::new();
  let run = RunId::generate();
  let target = browser
    .open_tab(run, &format!("{site}window-form"))
    .await
    .expect("opened");
  let tab = tab_of(&target);
  desktop
    .serve(
      &browser,
      BrowserCommand::Show {
        tab: tab.clone(),
        viewport: viewport(),
      },
    )
    .await;

  click(&desktop, &browser, &tab, (150, 120)).await;
  assert_eq!(
    evaluate(&browser, run, &target, "document.getElementById('button').textContent").await,
    "clicked"
  );

  click(&desktop, &browser, &tab, (150, 20)).await;
  let typed = PersonInput::Text {
    text: "hello".to_owned(),
  };
  desktop
    .serve(
      &browser,
      BrowserCommand::Input {
        tab: tab.clone(),
        input: typed,
      },
    )
    .await;
  key(&desktop, &browser, &tab, "Backspace").await;
  let pressed = PersonInput::Key {
    action: KeyAction::Down,
    key: "!".to_owned(),
    code: "Digit1".to_owned(),
    text: Some("!".to_owned()),
    modifiers: Modifiers {
      shift: true,
      ..Modifiers::default()
    },
  };
  desktop
    .serve(
      &browser,
      BrowserCommand::Input {
        tab: tab.clone(),
        input: pressed,
      },
    )
    .await;
  assert_eq!(
    evaluate(&browser, run, &target, "document.getElementById('field').value").await,
    "hell!"
  );
  browser.stop().await;
}

/// A press on a `<select>` is not sent to the page: the window gets the list to draw, and the person's choice is set
/// with the events the page listens to.
#[tokio::test(flavor = "multi_thread")]
async fn browser_select_list_is_drawn_by_the_window_and_its_choice_set() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let run = RunId::generate();
  let target = browser
    .open_tab(run, &format!("{site}window-form"))
    .await
    .expect("opened");
  let tab = tab_of(&target);
  desktop
    .serve(
      &browser,
      BrowserCommand::Show {
        tab: tab.clone(),
        viewport: viewport(),
      },
    )
    .await;

  let pressed = mouse(MouseAction::Pressed, 150, 215);
  desktop
    .serve(
      &browser,
      BrowserCommand::Input {
        tab: tab.clone(),
        input: pressed,
      },
    )
    .await;
  let BrowserEvent::SelectOpened { tab: opened, list } = desktop
    .wait_for(|event| matches!(event, BrowserEvent::SelectOpened { .. }))
    .await
  else {
    unreachable!("waited for a list");
  };
  assert_eq!(opened, tab);
  let labels: Vec<&str> = list.options.iter().map(|option| option.label.as_str()).collect();
  assert_eq!(labels, ["Alpha", "Beta", "Gamma", "Delta"]);
  assert!(list.options[0].selected && list.options[2].disabled);
  assert_eq!(
    (list.element.x, list.element.y, list.element.width, list.element.height),
    (0, 200, 300, 30)
  );

  let refused = browser
    .serve(
      BrowserCommand::ChooseOption {
        tab: tab.clone(),
        select: list.select,
        index: 2,
      },
      &desktop.sender,
    )
    .await;
  assert!(
    matches!(refused, Err(Rejection::Failed { .. })),
    "a disabled option: {refused:?}"
  );
  desktop
    .serve(
      &browser,
      BrowserCommand::ChooseOption {
        tab,
        select: list.select,
        index: 3,
      },
    )
    .await;
  assert_eq!(evaluate(&browser, run, &target, "document.title").await, "chose d");
  browser.stop().await;
}

/// A dialog of the person's tab is the window's to draw and answer.
#[tokio::test(flavor = "multi_thread")]
async fn browser_dialog_of_the_persons_tab_is_answered_by_the_window() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let tab = person_tab(&desktop, &browser, Some(format!("{site}dialog"))).await;

  let BrowserEvent::DialogOpened { tab: asking, dialog } = desktop
    .wait_for(|event| matches!(event, BrowserEvent::DialogOpened { .. }))
    .await
  else {
    unreachable!("waited for a dialog");
  };
  assert_eq!(asking, tab);
  assert_eq!(dialog.kind, DialogKind::Prompt);
  assert_eq!(
    (dialog.message.as_str(), dialog.default_prompt.as_str()),
    ("How many?", "3")
  );
  let answer = BrowserCommand::AnswerDialog {
    tab: tab.clone(),
    accept: true,
    text: Some("7".to_owned()),
  };
  desktop.serve(&browser, answer).await;
  desktop
    .wait_for(|event| matches!(event, BrowserEvent::DialogClosed { tab: closed } if *closed == tab))
    .await;
  // The page sets its title after the answer, by script: the window learns it without a navigation.
  desktop
    .state(|state| state.tabs.iter().any(|listed| listed.title == "answered 7"))
    .await;
  browser.stop().await;
}

/// The person's tabs are listed for the window with their owner, and no run sees or reaches them; a run's end leaves
/// them open. A run's first tab tells the window that an agent started browsing, once per run.
#[tokio::test(flavor = "multi_thread")]
async fn browser_persons_tabs_stay_invisible_to_runs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let run = RunId::generate();
  let runs_tab = browser.open_tab(run, &format!("{site}agent")).await.expect("opened");
  browser.open_tab(run, &format!("{site}again")).await.expect("opened");
  let persons = person_tab(&desktop, &browser, Some(format!("{site}mine"))).await;
  let persons_target = TargetId::from(persons.as_str());

  let listed: Vec<String> = browser
    .tabs(run)
    .await
    .expect("tabs")
    .into_iter()
    .map(|tab| tab.title)
    .collect();
  assert_eq!(listed, ["Page agent", "Page again"], "the run lists only its own tabs");
  let refused = browser
    .call(run, &persons_target, "Runtime.evaluate", json!({ "expression": "1" }))
    .await;
  assert!(matches!(refused, Err(BrowserError::NotRunsTab(_))), "{refused:?}");
  let state = desktop
    .state(|state| state.tabs.len() == 3 && state.tabs.iter().all(|tab| tab.title.starts_with("Page")))
    .await;
  let owners: Vec<TabOwner> = state.tabs.iter().map(|tab| tab.owner).collect();
  assert_eq!(owners, [TabOwner::Run { run }, TabOwner::Run { run }, TabOwner::Person]);
  let started: Vec<&BrowserEvent> = desktop
    .received()
    .iter()
    .filter(|event| matches!(event, BrowserEvent::AgentStartedBrowsing { .. }))
    .collect();
  assert_eq!(started, [&BrowserEvent::AgentStartedBrowsing { run }], "once per run");

  let observer = observer(profile.path()).await;
  browser.close_run_tabs(run).await.expect("closed");
  desktop.state(|state| state.tabs.len() == 1).await;
  assert!(
    open_in_browser(&observer, &persons_target).await,
    "the person's tab stays"
  );
  closed(&observer, &runs_tab).await;
  browser.stop().await;
}

/// While the desktop does not take the frames of a page that changes all the time, only the latest waits: the ones
/// before it are dropped, never queued.
#[tokio::test(flavor = "multi_thread")]
async fn browser_frames_are_dropped_rather_than_queued() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let desktop = Desktop::new();
  let tab = person_tab(&desktop, &browser, Some(format!("{site}animated"))).await;
  desktop
    .serve(
      &browser,
      BrowserCommand::Show {
        tab,
        viewport: viewport(),
      },
    )
    .await;
  let first = tokio::time::timeout(PATIENCE, browser.next_frame())
    .await
    .expect("a frame in time");

  let deadline = tokio::time::Instant::now() + PATIENCE;
  while browser.dropped_frames() < 3 && tokio::time::Instant::now() < deadline {
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  let dropped = browser.dropped_frames();
  assert!(dropped >= 3, "frames waiting were replaced: {dropped}");
  let latest = tokio::time::timeout(Duration::from_millis(100), browser.next_frame())
    .await
    .expect("the latest frame waits");
  assert!(
    latest.sequence > first.sequence + dropped,
    "only the latest of {dropped} frames reached the desktop: {} after {}",
    latest.sequence,
    first.sequence
  );
  browser.stop().await;
}

/// The person's address line and history, and the restart that reopens the tabs by their addresses for their owners.
#[tokio::test(flavor = "multi_thread")]
async fn browser_person_navigates_and_the_restart_reopens_the_tabs() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let browser = ProjectBrowser::new(profile.path().to_path_buf());
  let site = pages().await;
  let mut desktop = Desktop::new();
  desktop.serve(&browser, BrowserCommand::Subscribe).await;
  let run = RunId::generate();
  browser.open_tab(run, &format!("{site}kept")).await.expect("opened");
  let tab = person_tab(&desktop, &browser, Some(format!("{site}first"))).await;
  let at = |title: &'static str| move |state: &BrowserState| state.tabs.iter().any(|tab| tab.title == title);
  desktop.state(at("Page first")).await;

  let to = Navigation::Address {
    url: format!("{site}second"),
  };
  desktop
    .serve(&browser, BrowserCommand::Navigate { tab: tab.clone(), to })
    .await;
  desktop.state(at("Page second")).await;
  let back = BrowserCommand::Navigate {
    tab: tab.clone(),
    to: Navigation::Back,
  };
  desktop.serve(&browser, back).await;
  desktop.state(at("Page first")).await;
  let forward = BrowserCommand::Navigate {
    tab: tab.clone(),
    to: Navigation::Forward,
  };
  desktop.serve(&browser, forward.clone()).await;
  desktop.state(at("Page second")).await;
  let none = browser.serve(forward, &desktop.sender).await;
  assert!(
    matches!(none, Err(Rejection::Failed { .. })),
    "no page forward: {none:?}"
  );

  desktop.serve(&browser, BrowserCommand::BackToOrchester).await;
  let state = desktop
    .state(|state| {
      state.tabs.len() == 2
        && state
          .tabs
          .iter()
          .all(|listed| listed.title.starts_with("Page") && listed.id != tab)
    })
    .await;
  let reopened: Vec<(TabOwner, &str)> = state.tabs.iter().map(|tab| (tab.owner, tab.title.as_str())).collect();
  assert_eq!(
    reopened,
    [(TabOwner::Run { run }, "Page kept"), (TabOwner::Person, "Page second")]
  );
  assert_eq!(browser.tabs(run).await.expect("tabs").len(), 1, "the run keeps its tab");
  browser.stop().await;
}
