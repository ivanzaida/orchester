//! Plays a [`Fixture`] into the session's Browser window state, as the project's runner would report its browser: the
//! scene's tabs with the one shown, the page's frame and what else the runner reports. An agent starting to browse
//! opens the window, as the setting lets it by default. While the window is open the agent's action is reported again,
//! so it stays *acting*, an agent's wait is pushed back as a person who goes on using the tab would, and a runner that
//! reports its own browser over the fixture's is played over again.

use std::{path::Path, sync::Arc, time::Duration};

use orchester_ipc::browser::{
  BrowserEvent, BrowserMode, BrowserState as Reported, BrowserTab, BrowserTabId, ScreencastFrame, TabOwner,
};
use orchester_workspace::browser::{BrowserWindow, run_owners};
use tokio::{task::JoinHandle, time::sleep};

use super::{BrowserState, Fixture, tabs};
use crate::services::Services;

/// A page the computer's own browser rendered for the frames: a JPEG of `device` pixels showing `css` CSS pixels.
pub struct Page {
  pub jpeg: Vec<u8>,
  pub css: (u32, u32),
}

/// The page area of a 1280 × 800 window (`Page` `Dqgop`), and the line over it while an agent acts (`WTzV9`).
const PAGE_WIDTH: u32 = 1262;
const PAGE_HEIGHT: u32 = 710;
const ACTIVITY_HEIGHT: u32 = 34;
/// How often the agent's action is reported again, within the time it stays *acting*.
const AGAIN: Duration = Duration::from_secs(4);
const STEP: Duration = Duration::from_millis(400);

/// Plays `fixture` on the session's runtime; `render(query, (width, height))` renders the page with what its fields
/// hold in its `query` (`tabs::Scene::query`). The system's file dialog never opens: the stand-in for it never closes,
/// so the hint over a page that asks for a file stays (`03b` `TWpyp`).
pub fn play(
  services: &Services,
  fixture: Fixture,
  render: impl Fn(&str, (u32, u32)) -> Option<Page> + Send + 'static,
) -> JoinHandle<()> {
  if fixture.state == BrowserState::NotFound {
    services.browser.look_nowhere();
  }
  services
    .page_files
    .stand_in(|_| Box::pin(std::future::pending()));
  let window = services.runners.browser_window();
  services.runtime.spawn(async move {
    let runs = settled_first(&fixture).await;
    let scene = tabs::scene(fixture.state, &runs);
    let name: Arc<str> = Arc::from(fixture.name.as_str());
    let root = fixture.project.clone();
    let feed = |event: BrowserEvent| window.receive_for_test(&root, &name, event);
    other_project(&window, &root);
    feed(BrowserEvent::Changed(scene.reported.clone()));
    open(&window, &root, runs.first().copied(), &feed).await;
    let height = if matches!(fixture.state, BrowserState::Acting | BrowserState::Waiting) {
      PAGE_HEIGHT - ACTIVITY_HEIGHT
    } else {
      PAGE_HEIGHT
    };
    let query = scene.query();
    let page = tokio::task::spawn_blocking(move || render(&query, (PAGE_WIDTH, height)))
      .await
      .ok()
      .flatten();
    let frame = |sequence| {
      let (Some(page), Some(tab)) = (&page, &scene.reported.shown) else {
        return None;
      };
      Some(ScreencastFrame {
        tab: tab.clone(),
        sequence,
        css_width: page.css.0,
        css_height: page.css.1,
        jpeg: page.jpeg.clone(),
      })
    };
    let mut sequence = 1;
    if let Some(frame) = frame(sequence) {
      window.receive_frame_for_test(&root, frame);
    }
    for event in &scene.events {
      feed(event.clone());
    }
    loop {
      sleep(AGAIN).await;
      if window.shown_project().is_none() {
        continue;
      }
      let tabs: Vec<BrowserTab> = window.view(&root).map(|view| view.tabs().to_vec()).unwrap_or_default();
      if tabs != scene.reported.tabs {
        tracing::info!("the QA browser plays its tabs again over the runner's");
        feed(BrowserEvent::Changed(scene.reported.clone()));
        sequence += 1;
        if let Some(frame) = frame(sequence) {
          window.receive_frame_for_test(&root, frame);
        }
      }
      for event in &scene.events {
        match event {
          BrowserEvent::AgentActed { .. } => feed(event.clone()),
          BrowserEvent::AgentWaiting { run, tab, .. } => feed(BrowserEvent::AgentWaiting {
            run: *run,
            tab: tab.clone(),
            until: tabs::until_after_input(),
          }),
          _ => {}
        }
      }
    }
  })
}

/// The fixture's runs, those that do not wait for the person first: their tabs show no amber dot.
async fn settled_first(fixture: &Fixture) -> Vec<orchester_core::id::RunId> {
  let (root, runs) = (fixture.project.clone(), fixture.runs.clone());
  let owners = tokio::task::spawn_blocking(move || run_owners(&root, &runs)).await;
  let mut runs = fixture.runs.clone();
  match owners {
    Ok(Ok(owners)) => runs.sort_by_key(|run| owners.get(run).is_some_and(|owner| owner.waits)),
    Ok(Err(error)) => tracing::warn!(%error, "the QA browser cannot read its runs"),
    Err(error) => tracing::warn!(%error, "reading the QA browser's runs failed"),
  }
  runs
}

/// Reports an agent starting to browse until the window opened on the project.
async fn open(
  window: &BrowserWindow,
  root: &Path,
  run: Option<orchester_core::id::RunId>,
  feed: &impl Fn(BrowserEvent),
) {
  let run = run.unwrap_or_else(orchester_core::id::RunId::generate);
  for _ in 0..150 {
    if window.shown_project().as_deref() == Some(root) {
      return;
    }
    feed(BrowserEvent::AgentStartedBrowsing { run });
    sleep(STEP).await;
  }
  tracing::warn!("the Browser window did not open for the QA browser");
}

/// Another project with tabs of the person's, for the project switcher (`01b` `f9Z3no`).
fn other_project(window: &BrowserWindow, root: &Path) {
  let other = root.with_file_name("orchester-qa");
  let tab = |id: &str, title: &str, url: &str| BrowserTab {
    id: BrowserTabId::new(id),
    owner: TabOwner::Person,
    title: title.to_owned(),
    url: url.to_owned(),
    loading: false,
  };
  window.receive_for_test(
    &other,
    "Orchester",
    BrowserEvent::Changed(Reported {
      tabs: vec![
        tab("qa-other-1", "docs.rs", "https://docs.rs/"),
        tab(
          "qa-other-2",
          "Releases · GitHub",
          "https://github.com/ivanzaida/orchester/releases",
        ),
      ],
      shown: None,
      mode: BrowserMode::Headless,
    }),
  );
}
