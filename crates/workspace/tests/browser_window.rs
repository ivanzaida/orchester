//! The *Browser* window's state (`orchester_workspace::browser`): each project's browser as its runner reports it,
//! the frames of the tab shown decoded to RGBA off the UI thread, the rule that opens the window when an agent
//! starts browsing, and the tabs' owners ([`owners`]). The state is fed here as a runner's connection feeds it;
//! [`with_runner`] runs the real `orchester-runner` with this computer's Chrome, Edge, Brave or Chromium, and says so
//! and checks nothing where none is found. Run with `cargo test -p orchester-workspace --test browser_window
//! browser_window_`.

#[path = "browser_window/owners.rs"]
mod owners;
#[path = "support/runner_programs.rs"]
mod runner_programs;
mod support;
#[path = "browser_window/with_runner.rs"]
mod with_runner;

use std::{io::Cursor, path::Path, time::Duration};

use image::{ImageFormat, Rgb, RgbImage};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::browser::{
  AgentAction, AgentWaitEnd, BrowserDialog, BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId,
  DialogKind, ElementBox, FileChooser, FileChooserId, ScreencastFrame, SelectId, SelectList, SelectOption, TabOwner,
};
use orchester_store::AppDb;
use orchester_workspace::{
  browser::{AgentWaiting, BrowserView, BrowserWindow, StartedBrowsing},
  workspace::Workspace,
};
use tokio::time::timeout;

const PATIENCE: Duration = Duration::from_secs(20);

fn tab(id: &str) -> BrowserTabId {
  BrowserTabId::new(id)
}

fn browser_tab(id: &str, owner: TabOwner) -> BrowserTab {
  BrowserTab {
    id: tab(id),
    owner,
    title: format!("Page {id}"),
    url: format!("http://127.0.0.1/{id}"),
    loading: false,
  }
}

fn state(tabs: Vec<BrowserTab>, shown: Option<&str>) -> BrowserEvent {
  BrowserEvent::Changed(BrowserState {
    tabs,
    shown: shown.map(tab),
    mode: BrowserMode::Headless,
  })
}

/// A JPEG of `width` × `height` pixels of one color, as the browser's screencast sends.
fn jpeg(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
  let mut bytes = Cursor::new(Vec::new());
  RgbImage::from_pixel(width, height, Rgb(color))
    .write_to(&mut bytes, ImageFormat::Jpeg)
    .expect("encode a JPEG");
  bytes.into_inner()
}

fn frame(id: &str, sequence: u64, jpeg: Vec<u8>) -> ScreencastFrame {
  ScreencastFrame {
    tab: tab(id),
    sequence,
    css_width: 160,
    css_height: 100,
    jpeg,
  }
}

/// Waits until the view of the project at `root` is as `wanted` says.
pub async fn view_until(window: &BrowserWindow, root: &Path, wanted: impl Fn(&BrowserView) -> bool) -> BrowserView {
  let mut changes = window.changes();
  timeout(PATIENCE, async {
    loop {
      if let Some(view) = window.view(root).filter(|view| wanted(view)) {
        return view;
      }
      changes.changed().await.expect("the window's state outlives the test");
    }
  })
  .await
  .expect("the window's state in time")
}

/// Whether `pixels` (RGBA) are `color` at pixel `index`, within the JPEG's loss.
pub fn near(pixels: &[u8], index: usize, color: [u8; 3]) -> bool {
  let at = &pixels[index * 4..index * 4 + 4];
  at[3] == 255 && at[..3].iter().zip(color).all(|(got, wanted)| got.abs_diff(wanted) <= 8)
}

#[test]
fn browser_window_lists_the_projects_whose_browser_has_tabs_by_name() {
  let window = BrowserWindow::default();
  let (kontur, atlas, idle) = (Path::new("/p/kontur"), Path::new("/p/atlas"), Path::new("/p/idle"));
  let run = RunId::generate();
  window.receive_for_test(
    kontur,
    "Kontur",
    state(
      vec![
        browser_tab("T1", TabOwner::Run { run }),
        browser_tab("T2", TabOwner::Person),
      ],
      Some("T2"),
    ),
  );
  window.receive_for_test(atlas, "Atlas", state(vec![browser_tab("A1", TabOwner::Person)], None));
  window.receive_for_test(idle, "Idle", state(Vec::new(), None));

  let projects = window.projects();
  let listed: Vec<(&Path, &str, usize)> = projects
    .iter()
    .map(|(root, view)| (root.as_path(), view.name(), view.tabs().len()))
    .collect();
  assert_eq!(listed, [(atlas, "Atlas", 1), (kontur, "Kontur", 2)]);
  let kontur_view = window.view(kontur).expect("Kontur's browser");
  assert_eq!(kontur_view.shown(), Some(&tab("T2")));
  assert_eq!(kontur_view.tabs()[0].owner, TabOwner::Run { run });
  assert_eq!(window.view(idle).expect("Idle's browser").tabs(), []);
}

fn typed() -> AgentAction {
  AgentAction {
    tool: "browser_type".to_owned(),
    summary: "typed 8 in Max attempts".to_owned(),
    element: Some(ElementBox {
      x: 10,
      y: 20,
      width: 100,
      height: 24,
    }),
  }
}

fn leave() -> BrowserDialog {
  BrowserDialog {
    kind: DialogKind::Confirm,
    message: "Leave?".to_owned(),
    default_prompt: String::new(),
  }
}

fn schedule() -> SelectList {
  SelectList {
    select: SelectId::new(7),
    options: vec![SelectOption {
      label: "Daily".to_owned(),
      selected: true,
      disabled: false,
    }],
    element: ElementBox {
      x: 0,
      y: 0,
      width: 80,
      height: 20,
    },
  }
}

#[test]
fn browser_window_keeps_what_belongs_to_a_tab_until_it_closes() {
  let window = BrowserWindow::default();
  let root = Path::new("/p/kontur");
  let run = RunId::generate();
  let tabs = vec![
    browser_tab("T1", TabOwner::Run { run }),
    browser_tab("T2", TabOwner::Person),
  ];
  window.receive_for_test(root, "Kontur", state(tabs, Some("T1")));
  let (action, dialog, list) = (typed(), leave(), schedule());
  let events = [
    BrowserEvent::AgentActed {
      run,
      tab: tab("T1"),
      action: action.clone(),
    },
    BrowserEvent::DialogOpened {
      tab: tab("T2"),
      dialog: dialog.clone(),
    },
    BrowserEvent::SelectOpened {
      tab: tab("T2"),
      list: list.clone(),
    },
  ];
  for event in events {
    window.receive_for_test(root, "Kontur", event);
  }

  let view = window.view(root).expect("the browser");
  let acting = view.acting(&tab("T1")).expect("the agent's last action");
  assert_eq!((acting.run, &acting.action), (run, &action));
  assert_eq!(view.latest_acting().map(|acting| acting.run), Some(run));
  assert_eq!(view.acting(&tab("T2")), None);
  assert_eq!(view.dialog(&tab("T2")), Some(&dialog));
  assert_eq!(
    view.select().map(|select| (&select.tab, &select.list)),
    Some((&tab("T2"), &list))
  );

  window.receive_for_test(root, "Kontur", BrowserEvent::DialogClosed { tab: tab("T2") });
  window.close_select(root);
  let view = window.view(root).expect("the browser");
  assert_eq!((view.dialog(&tab("T2")), view.select()), (None, None));

  // The window stops showing: T1 is still the tab to show again. Then the run's tab closes, and all of it goes.
  window.receive_for_test(
    root,
    "Kontur",
    state(
      vec![
        browser_tab("T1", TabOwner::Run { run }),
        browser_tab("T2", TabOwner::Person),
      ],
      None,
    ),
  );
  assert_eq!(window.view(root).expect("the browser").last_shown(), Some(&tab("T1")));
  window.receive_for_test(root, "Kontur", state(vec![browser_tab("T2", TabOwner::Person)], None));
  let view = window.view(root).expect("the browser");
  assert_eq!((view.acting(&tab("T1")), view.latest_acting()), (None, None));
  assert_eq!(view.last_shown(), None);
}

/// A request of the person's tab for files stays until the window answers it, whose answer to an earlier request
/// leaves a later one; it goes with its tab.
#[test]
fn browser_window_keeps_a_tabs_request_for_files_until_the_window_answers_it() {
  let window = BrowserWindow::default();
  let root = Path::new("/p/kontur");
  let tabs = || vec![browser_tab("P1", TabOwner::Person), browser_tab("P2", TabOwner::Person)];
  window.receive_for_test(root, "Kontur", state(tabs(), Some("P1")));
  let asked = |id: u64| FileChooser {
    id: FileChooserId::new(id),
    multiple: false,
    accept: vec![".pem".to_owned()],
    label: Some("Signing certificate".to_owned()),
  };
  for (on, id) in [("P1", 1), ("P1", 2), ("P2", 3)] {
    let opened = BrowserEvent::FileChooserOpened {
      tab: tab(on),
      chooser: asked(id),
    };
    window.receive_for_test(root, "Kontur", opened);
  }
  let view = window.view(root).expect("the browser");
  assert_eq!(view.file_chooser(&tab("P1")), Some(&asked(2)), "the later request");
  assert_eq!(view.file_choosers().count(), 2);

  window.file_chooser_answered(root, &tab("P1"), FileChooserId::new(1));
  let view = window.view(root).expect("the browser");
  assert_eq!(view.file_chooser(&tab("P1")), Some(&asked(2)), "an earlier answer leaves it");
  window.file_chooser_answered(root, &tab("P1"), FileChooserId::new(2));
  let view = window.view(root).expect("the browser");
  assert_eq!(view.file_chooser(&tab("P1")), None);

  window.receive_for_test(root, "Kontur", state(vec![browser_tab("P1", TabOwner::Person)], Some("P1")));
  let view = window.view(root).expect("the browser");
  assert_eq!(view.file_choosers().count(), 0, "gone with its tab");
}

/// Each tab keeps whether an agent waits on it and until when: pushed back, ended whichever way, and gone with its tab.
#[test]
fn browser_window_keeps_the_agent_waiting_on_each_tab_until_its_wait_ends() {
  let window = BrowserWindow::default();
  let root = Path::new("/p/kontur");
  let run = RunId::generate();
  let tabs = |ids: &[&str]| {
    ids
      .iter()
      .map(|id| browser_tab(id, TabOwner::Run { run }))
      .collect::<Vec<_>>()
  };
  window.receive_for_test(root, "Kontur", state(tabs(&["T1", "T2"]), Some("T1")));
  let moment = Timestamp::from_unix_millis;
  let waits = |tab_id: &str, until| BrowserEvent::AgentWaiting {
    run,
    tab: tab(tab_id),
    until,
  };

  let view = window.view(root).expect("the browser");
  assert_eq!(view.waiting(&tab("T1")), None, "no agent waits yet");

  window.receive_for_test(root, "Kontur", waits("T1", moment(10_000)));
  window.receive_for_test(root, "Kontur", waits("T2", moment(20_000)));
  let view = window.view(root).expect("the browser");
  assert_eq!(
    view.waiting(&tab("T1")),
    Some(&AgentWaiting {
      run,
      until: moment(10_000)
    })
  );
  assert_eq!(
    view.waiting(&tab("T2")).map(|waiting| waiting.until),
    Some(moment(20_000))
  );

  window.receive_for_test(root, "Kontur", waits("T1", moment(14_000)));
  let pushed = window.view(root).expect("the browser");
  assert_eq!(
    pushed.waiting(&tab("T1")).map(|waiting| waiting.until),
    Some(moment(14_000)),
    "new input pushes it back"
  );

  for (at, end) in [
    (1, AgentWaitEnd::Ran),
    (2, AgentWaitEnd::Capped),
    (3, AgentWaitEnd::Cancelled),
  ] {
    window.receive_for_test(root, "Kontur", waits("T1", moment(at)));
    window.receive_for_test(
      root,
      "Kontur",
      BrowserEvent::AgentWaitEnded {
        run,
        tab: tab("T1"),
        end,
      },
    );
    let view = window.view(root).expect("the browser");
    assert_eq!(view.waiting(&tab("T1")), None, "{end:?} ends the wait on T1");
    assert!(view.waiting(&tab("T2")).is_some(), "and not on T2");
  }

  window.receive_for_test(root, "Kontur", state(tabs(&["T1"]), Some("T1")));
  assert_eq!(
    window.view(root).expect("the browser").waiting(&tab("T2")),
    None,
    "it goes with its tab"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_window_decodes_the_latest_frame_of_the_tab_shown_to_rgba() {
  let window = BrowserWindow::default();
  let root = Path::new("/p/kontur");
  let tabs = vec![browser_tab("T1", TabOwner::Person), browser_tab("T2", TabOwner::Person)];
  window.receive_for_test(root, "Kontur", state(tabs.clone(), Some("T1")));

  window.receive_frame_for_test(root, frame("T1", 5, jpeg(160, 100, [0, 128, 255])));
  let view = view_until(&window, root, |view| view.frame().is_some()).await;
  let decoded = view.frame().expect("the frame");
  assert_eq!((decoded.width(), decoded.height(), decoded.sequence()), (160, 100, 5));
  assert_eq!(decoded.css_size(), (160, 100));
  assert_eq!(decoded.pixels().len(), 160 * 100 * 4);
  assert!(near(decoded.pixels(), 50 * 160 + 80, [0, 128, 255]), "the page's color");

  // Bytes that are no JPEG are dropped; the frame before stays, and the next frame is decoded.
  window.receive_frame_for_test(root, frame("T1", 6, b"not a jpeg".to_vec()));
  window.receive_frame_for_test(root, frame("T1", 7, jpeg(160, 100, [255, 0, 0])));
  let view = view_until(&window, root, |view| {
    view.frame().is_some_and(|frame| frame.sequence() == 7)
  })
  .await;
  assert!(near(view.frame().expect("the frame").pixels(), 0, [255, 0, 0]));

  // An earlier frame decoded late does not replace a later one.
  window.receive_frame_for_test(root, frame("T1", 3, jpeg(16, 10, [0, 0, 0])));
  window.receive_frame_for_test(root, frame("T1", 8, jpeg(16, 10, [0, 255, 0])));
  let view = view_until(&window, root, |view| {
    view.frame().is_some_and(|frame| frame.sequence() == 8)
  })
  .await;
  assert_eq!(view.frame().expect("the frame").width(), 16);

  // Another tab shown: the frame of T1 is not given for it.
  window.receive_for_test(root, "Kontur", state(tabs, Some("T2")));
  assert!(window.view(root).expect("the browser").frame().is_none());
}

/// Frames sent faster than they decode: each decode counts one change of the window's state, so the count says how
/// many were decoded.
#[tokio::test(flavor = "multi_thread")]
async fn browser_window_drops_frames_while_one_decodes_instead_of_queueing_them() {
  let window = BrowserWindow::default();
  let root = Path::new("/p/kontur");
  window.receive_for_test(
    root,
    "Kontur",
    state(vec![browser_tab("T1", TabOwner::Person)], Some("T1")),
  );
  let before = *window.changes().borrow();
  let page = jpeg(1600, 1000, [0, 128, 255]);
  for sequence in 0..40 {
    window.receive_frame_for_test(root, frame("T1", sequence, page.clone()));
  }
  view_until(&window, root, |view| {
    view.frame().is_some_and(|frame| frame.sequence() == 39)
  })
  .await;
  let decodes = *window.changes().borrow() - before;
  assert!(decodes < 40, "frames were dropped, not all decoded: {decodes} decodes");
}

#[test]
fn browser_window_opens_by_itself_only_while_closed_and_allowed() {
  let data = tempfile::tempdir().expect("temporary directory");
  let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("the app database"));
  let window = BrowserWindow::default();
  let (kontur, atlas) = (Path::new("/p/kontur"), Path::new("/p/atlas"));
  let run = RunId::generate();
  let mut started = window.started();
  assert_eq!(*started.borrow_and_update(), None);

  window.receive_for_test(kontur, "Kontur", BrowserEvent::AgentStartedBrowsing { run });
  assert!(started.has_changed().expect("the window's state outlives the test"));
  let first = started.borrow_and_update().clone().expect("an agent started browsing");
  assert_eq!(
    first,
    StartedBrowsing {
      project: kontur.to_path_buf(),
      run
    }
  );
  assert!(window.opens_for(&first, &workspace).expect("read"), "on by default");

  workspace.set_browser_window_opens_itself(false).expect("turn off");
  assert!(!window.opens_for(&first, &workspace).expect("read"), "turned off");
  workspace.set_browser_window_opens_itself(true).expect("turn on");

  // Open on Kontur, it does not jump to Atlas when an agent starts browsing there.
  window.showing(kontur);
  let other = StartedBrowsing {
    project: atlas.to_path_buf(),
    run: RunId::generate(),
  };
  assert!(!window.opens_for(&other, &workspace).expect("read"));
  assert_eq!(window.shown_project().as_deref(), Some(kontur));
  window.closed();
  assert!(window.opens_for(&other, &workspace).expect("read"));
}
