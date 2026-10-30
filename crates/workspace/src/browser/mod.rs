//! The *Browser* window's state (`docs/design/browser.md`, "The *Browser* window", "Where the code goes"): each
//! project's browser as its runner reports it ([`BrowserView`]), the latest frame of the tab shown decoded off the UI
//! thread ([`RgbaFrame`]), which project the window shows, and the rule that opens the window by itself when an agent
//! starts browsing ([`BrowserWindow::opens_for`], decision 14). The window's requests go to the project's runner
//! ([`crate::runner::Runners::browser`]).
//!
//! Frames are decoded one at a time per project: a frame that arrives while one decodes waits alone, and a newer one
//! replaces it, so a slow decode drops frames instead of queueing them, as the runner does before sending.

mod frame;
mod owners;
mod view;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::id::RunId;
use orchester_ipc::browser::{BrowserEvent, ScreencastFrame};
#[cfg(feature = "test-support")]
use orchester_ipc::{
  Rejection, Reply,
  browser::{BrowserCommand, BrowserReply},
};
use orchester_store::StoreError;
use parking_lot::Mutex;
#[cfg(feature = "test-support")]
use tokio::sync::oneshot;
use tokio::sync::watch;

pub use self::{
  frame::{FrameDecodeError, RgbaFrame},
  owners::{RunOwner, run_owners},
  view::{AgentActing, BrowserView, OpenSelect},
};
#[cfg(feature = "test-support")]
use crate::runner::RequestError;
use crate::workspace::Workspace;

/// An agent of `run` opened the run's first tab in the browser of the project at `project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartedBrowsing {
  pub project: PathBuf,
  pub run: RunId,
}

/// The window's state, shared by the runners' connections that fill it and the window that shows it. Clones share it.
#[derive(Clone)]
pub struct BrowserWindow {
  inner: Arc<Inner>,
}

impl Default for BrowserWindow {
  fn default() -> Self {
    Self {
      inner: Arc::new(Inner {
        projects: Mutex::new(HashMap::new()),
        showing: Mutex::new(None),
        changes: watch::Sender::new(0),
        started: watch::Sender::new(None),
        #[cfg(feature = "test-support")]
        recorded: Mutex::new(None),
      }),
    }
  }
}

struct Inner {
  projects: Mutex<HashMap<PathBuf, Project>>,
  /// The project the window shows; `None` while it is closed.
  showing: Mutex<Option<PathBuf>>,
  /// Counts the changes of every project's browser and of its frames.
  changes: watch::Sender<u64>,
  started: watch::Sender<Option<StartedBrowsing>>,
  /// The window's requests, kept instead of sent while a test records them.
  #[cfg(feature = "test-support")]
  recorded: Mutex<Option<Recorded>>,
}

/// What a test keeps of the window's requests.
#[cfg(feature = "test-support")]
#[derive(Default)]
struct Recorded {
  /// Each with its project.
  commands: Vec<(PathBuf, BrowserCommand)>,
  /// A request waits for its answer in `held`, instead of being answered as it is kept.
  holding: bool,
  held: Vec<oneshot::Sender<Result<Reply, RequestError>>>,
}

struct Project {
  view: BrowserView,
  decoding: Decoding,
}

/// The frames of one project on their way to RGBA.
#[derive(Default)]
struct Decoding {
  busy: bool,
  waiting: Option<ScreencastFrame>,
}

impl BrowserWindow {
  /// Changes whenever a project's browser or the frame of its tab shown changes.
  pub fn changes(&self) -> watch::Receiver<u64> {
    self.inner.changes.subscribe()
  }

  /// The project at `root`'s browser, once its runner reported it.
  pub fn view(&self, root: &Path) -> Option<BrowserView> {
    self.inner.projects.lock().get(root).map(|project| project.view.clone())
  }

  /// The projects whose browser has tabs, by name: the window's project switcher.
  pub fn projects(&self) -> Vec<(PathBuf, BrowserView)> {
    let mut projects: Vec<(PathBuf, BrowserView)> = self
      .inner
      .projects
      .lock()
      .iter()
      .filter(|(_, project)| !project.view.tabs().is_empty())
      .map(|(root, project)| (root.clone(), project.view.clone()))
      .collect();
    projects.sort_by(|(_, left), (_, right)| left.name().cmp(right.name()));
    projects
  }

  /// The latest time an agent started browsing, whenever it changes: the window then asks [`Self::opens_for`].
  pub fn started(&self) -> watch::Receiver<Option<StartedBrowsing>> {
    self.inner.started.subscribe()
  }

  /// Whether the window opens by itself, showing `started`'s project (decision 14): only while it is closed, since
  /// once open it does not jump to another project by itself, and only if the person lets it (*Open the Browser
  /// window when an agent starts browsing*, Settings › Browser, on until turned off). Blocks on SQLite.
  pub fn opens_for(&self, started: &StartedBrowsing, workspace: &Workspace) -> Result<bool, StoreError> {
    if self.inner.showing.lock().is_some() {
      tracing::debug!(project = %started.project.display(), "the Browser window is open already");
      return Ok(false);
    }
    workspace.browser_window_opens_itself()
  }

  /// The window shows the project at `root`, opened or switched to it.
  pub fn showing(&self, root: &Path) {
    *self.inner.showing.lock() = Some(root.to_path_buf());
  }

  /// The window closed.
  pub fn closed(&self) {
    *self.inner.showing.lock() = None;
  }

  /// The project the window shows; `None` while it is closed.
  pub fn shown_project(&self) -> Option<PathBuf> {
    self.inner.showing.lock().clone()
  }

  /// The `<select>` list of the project at `root` closed: the person chose an option or dismissed it.
  pub fn close_select(&self, root: &Path) {
    let closed = self
      .inner
      .projects
      .lock()
      .get_mut(root)
      .is_some_and(|project| project.view.close_select());
    if closed {
      self.changed();
    }
  }

  /// The runner of the project at `root`, named `name`, reported `event`.
  pub(crate) fn receive(&self, root: &Path, name: &Arc<str>, event: BrowserEvent) {
    if let BrowserEvent::AgentStartedBrowsing { run } = event {
      self.inner.started.send_replace(Some(StartedBrowsing {
        project: root.to_path_buf(),
        run,
      }));
    }
    self
      .inner
      .projects
      .lock()
      .entry(root.to_path_buf())
      .or_insert_with(|| Project {
        view: BrowserView::new(Arc::clone(name)),
        decoding: Decoding::default(),
      })
      .view
      .apply(event);
    self.changed();
  }

  /// A frame of the tab the window shows in the project at `root`: decoded on a blocking thread of the runtime, or
  /// kept to be decoded next, replacing a frame kept before. Needs a tokio runtime.
  pub(crate) fn receive_frame(&self, root: &Path, frame: ScreencastFrame) {
    {
      let mut projects = self.inner.projects.lock();
      let Some(project) = projects.get_mut(root) else {
        tracing::debug!(project = %root.display(), "a frame before the browser's state; dropped");
        return;
      };
      if project.decoding.busy {
        project.decoding.waiting = Some(frame);
        return;
      }
      project.decoding.busy = true;
    }
    let (window, root) = (self.clone(), root.to_path_buf());
    tokio::task::spawn_blocking(move || {
      let mut next = Some(frame);
      while let Some(frame) = next {
        let decoded = RgbaFrame::decode(&frame);
        next = window.decoded(&root, decoded);
      }
    });
  }

  /// Keeps a decoded frame; the frame to decode next, if one waits.
  fn decoded(&self, root: &Path, decoded: Result<RgbaFrame, FrameDecodeError>) -> Option<ScreencastFrame> {
    let next = {
      let mut projects = self.inner.projects.lock();
      let project = projects.get_mut(root)?;
      match decoded {
        Ok(frame) => project.view.decoded(frame),
        Err(error) => tracing::warn!(project = %root.display(), %error, "a screencast frame was dropped"),
      }
      let next = project.decoding.waiting.take();
      project.decoding.busy = next.is_some();
      next
    };
    self.changed();
    next
  }

  /// The runner of the project at `root` is gone, and its browser with it.
  pub(crate) fn ended(&self, root: &Path) {
    if self.inner.projects.lock().remove(root).is_some() {
      self.changed();
    }
  }

  fn changed(&self) {
    self.inner.changes.send_modify(|revision| *revision += 1);
  }

  /// [`Self::receive`], for tests of the window's state, which run no runner.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn receive_for_test(&self, root: &Path, name: &str, event: BrowserEvent) {
    self.receive(root, &Arc::from(name), event);
  }

  /// [`Self::receive_frame`], for tests of the window's state, which run no runner.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn receive_frame_for_test(&self, root: &Path, frame: ScreencastFrame) {
    self.receive_frame(root, frame);
  }

  /// From now on the window's requests are kept, each answered as carried out, instead of sent to a runner: for tests
  /// of what the window asks, which run no runner.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn record_commands_for_test(&self) {
    self.inner.recorded.lock().get_or_insert_with(Recorded::default);
  }

  /// From now on a request kept waits for its answer until [`Self::answer_held_for_test`], as one waits for a runner:
  /// for tests of what the window does meanwhile.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn hold_answers_for_test(&self) {
    self.inner.recorded.lock().get_or_insert_with(Recorded::default).holding = true;
  }

  /// Gives every request waiting since [`Self::hold_answers_for_test`] `answer`.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn answer_held_for_test(&self, answer: &Result<BrowserReply, Rejection>) {
    let held = self
      .inner
      .recorded
      .lock()
      .as_mut()
      .map(|recorded| std::mem::take(&mut recorded.held))
      .unwrap_or_default();
    for reply in held {
      let answered = match answer.clone() {
        Ok(reply) => Reply::Browser(reply),
        Err(rejection) => Reply::Rejected(rejection),
      };
      // A request nobody waits for any more needs no answer.
      drop(reply.send(Ok(answered)));
    }
  }

  /// The requests kept since the last call, each with the project it was for.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn recorded_commands_for_test(&self) -> Vec<(PathBuf, BrowserCommand)> {
    self
      .inner
      .recorded
      .lock()
      .as_mut()
      .map(|recorded| std::mem::take(&mut recorded.commands))
      .unwrap_or_default()
  }

  /// Keeps `command` for the project at `root` while a test records the requests, and gives where its answer comes;
  /// gives it back otherwise.
  #[cfg(feature = "test-support")]
  pub(crate) fn record(
    &self,
    root: &Path,
    command: BrowserCommand,
  ) -> Result<oneshot::Receiver<Result<Reply, RequestError>>, BrowserCommand> {
    let mut recorded = self.inner.recorded.lock();
    let Some(recorded) = recorded.as_mut() else {
      return Err(command);
    };
    recorded.commands.push((root.to_path_buf(), command));
    let (reply, answer) = oneshot::channel();
    if recorded.holding {
      recorded.held.push(reply);
    } else {
      // `answer` is right here to take it.
      drop(reply.send(Ok(Reply::Browser(BrowserReply::Done))));
    }
    Ok(answer)
  }
}
