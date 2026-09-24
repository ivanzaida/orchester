//! The whole desktop mounted headlessly on a temporary application database, driven by keys.
//! Every step runs a headless lurq pass (layout, modals included, without a window), so pages,
//! inspectors and dialogs are all reachable, and the focus is read from the tree.

use std::{
  fs,
  path::{Path, PathBuf},
  thread,
  time::Duration,
};

use lurq::app::{App, Tree, events::MouseButton};
use orchester_desktop::{
  activity::Activity,
  close::Lifecycle,
  i18n,
  project_setup::ProjectDraft,
  root::{Root, Startup},
  runner::Runners,
  session::AppSession,
  theme,
};
use tempfile::TempDir;
use tokio::runtime::Runtime;

const SETTLE_TRIES: usize = 400;
const SETTLE_PAUSE: Duration = Duration::from_millis(5);
/// The design's window size (`ZBoBm` and the other 1440 screens).
const WIDTH: u32 = 1440;
const HEIGHT: u32 = 960;

pub struct Desktop {
  pub tree: Tree,
  pub repo: PathBuf,
  app: App,
  _session: AppSession,
  _runtime: Runtime,
  _dir: TempDir,
}

impl Desktop {
  /// First launch: no project yet.
  pub fn first_launch() -> Self {
    Self::start(false)
  }

  /// A project in a git repository, opened at startup.
  pub fn with_project() -> Self {
    Self::start(true)
  }

  fn start(with_project: bool) -> Self {
    let dir = TempDir::new().expect("temporary directory");
    let repo = dir.path().join("kontur");
    fs::create_dir_all(repo.join(".git")).expect("create .git");
    let session = AppSession::start_at(&dir.path().join("orchester.db")).expect("session starts");
    if with_project {
      let new = ProjectDraft {
        name: "Kontur",
        repository: Some(&repo),
      }
      .validate()
      .expect("valid draft");
      session.workspace().create(new).expect("project is created");
    }
    let runtime = Runtime::new().expect("tokio runtime");
    let mut app = App::new();
    app.set_tokio_handle(runtime.handle().clone());
    theme::install(&mut app);
    i18n::install(&app).expect("bundled locales load");
    // No runner program exists here; the project shows its runner as unable to start.
    let activity = Activity::new();
    let runners = Runners::new(
      dir.path().join("no-runner.exe"),
      runtime.handle().clone(),
      activity.clone(),
    );
    let startup = Startup::Ready {
      workspace: session.workspace().clone(),
      lifecycle: Lifecycle::new(runners, runtime.handle().clone()),
      activity,
    };
    let mut tree = Tree::new();
    tree.resize(WIDTH, HEIGHT);
    tree.mount_root::<Root>(&mut app, startup);
    Self {
      tree,
      repo,
      app,
      _session: session,
      _runtime: runtime,
      _dir: dir,
    }
  }

  pub fn repo(&self) -> &Path {
    &self.repo
  }

  /// Presses Tab (or Shift+Tab) `count` times and reports, after each press, the focused
  /// element (see [`Desktop::focused`]).
  pub fn stops(&mut self, count: usize, reverse: bool) -> Vec<Option<String>> {
    (0..count)
      .map(|_| {
        if reverse {
          self.shift_tab();
        } else {
          self.tab();
        }
        self.focused()
      })
      .collect()
  }

  /// The id of the focused element; `None` when nothing has the focus or it has no id.
  pub fn focused(&self) -> Option<String> {
    self
      .tree
      .focused_element()
      .and_then(|element| element.id().map(str::to_owned))
  }

  /// Tabs until `id` has the focus, at most `limit` times.
  pub fn tab_to(&mut self, id: &str, limit: usize) {
    for _ in 0..limit {
      self.tab();
      if self.focused().as_deref() == Some(id) {
        return;
      }
    }
    panic!("Tab does not reach {id} in {limit} stops");
  }

  /// A headless pass: rebuilds what changed and lays out the window, modals included.
  pub fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  /// Lets background work finish until `done` holds.
  pub fn settle(&mut self, what: &str, done: impl Fn(&mut Tree) -> bool) {
    for _ in 0..SETTLE_TRIES {
      self.tree.tick_futures();
      self.pass();
      if done(&mut self.tree) {
        return;
      }
      thread::sleep(SETTLE_PAUSE);
    }
    panic!("timed out waiting for {what}");
  }

  pub fn wait_for(&mut self, id: &str) {
    self.settle(id, |tree| tree.get_element_by_id(id).is_some());
  }

  pub fn click(&mut self, id: &str) {
    self.tree.get_element_by_id_mut(id).expect(id).click();
    self.pass();
  }

  /// A pointer press and release in the middle of `id`, as a user clicks.
  pub fn press(&mut self, id: &str) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.pass();
  }

  /// A key press, then a pass so the next step sees what it changed.
  pub fn key(&mut self, key: &str, code: &str) {
    self.tree.key_down(key.into(), code.into(), false, false, false);
    self.pass();
  }

  pub fn tab(&mut self) {
    self.key("Tab", "Tab");
  }

  pub fn shift_tab(&mut self) {
    self.tree.key_down("Tab".into(), "Tab".into(), true, false, false);
    self.pass();
  }

  pub fn enter(&mut self) {
    self.key("Enter", "Enter");
  }

  pub fn value(&mut self, id: &str) -> String {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .map(|input| input.value())
      .unwrap_or_else(|| panic!("text input {id}"))
  }

  pub fn type_text(&mut self, text: &str) {
    for character in text.chars() {
      let key = character.to_string();
      self.key(&key, &key);
    }
  }
}

/// Expected [`Desktop::stops`], written with `&str`.
pub fn expected(stops: &[Option<&str>]) -> Vec<Option<String>> {
  stops.iter().map(|stop| stop.map(str::to_owned)).collect()
}
