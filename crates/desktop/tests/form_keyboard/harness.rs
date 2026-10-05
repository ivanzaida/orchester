//! The whole desktop mounted headlessly on a temporary application database, driven by keys.
//! Every step runs a headless lurq pass (layout, modals included, without a window), so pages,
//! inspectors and dialogs are all reachable, and the focus is read from the tree.

use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::{App, Tree, events::MouseButton, theme::PaletteColor},
  core::ElementRect,
  layout::quad::{Quad, QuadContent},
  mcp::McpConfig,
  node::color::Color,
};
use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_desktop::{
  activity::Activity,
  clock::Clock,
  close::Lifecycle,
  embedded_mcp::McpStatus,
  i18n,
  programs::Programs,
  project_setup::ProjectDraft,
  root::{Root, RootProps, Startup},
  runner::Runners,
  secrets::Vault,
  session::AppSession,
  theme,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::{mcp_agent::McpAgent, test_clock::TestClock};

/// How long a wait may take before the test fails. A wait ends as soon as what it waits for is
/// shown, so this only bounds a broken test. It is generous because the tests run side by side,
/// often next to builds: on a loaded machine background reads, the runner and its agents take
/// many times longer than on an idle one.
const PATIENCE: Duration = Duration::from_secs(120);
/// Between two looks at the tree, background work runs.
const POLL_PAUSE: Duration = Duration::from_millis(10);
/// Folder of the other project of [`Desktop::with_other_project`].
const OTHER_REPO: &str = "personal-website";
/// The design's window size (`ZBoBm` and the other 1440 screens).
const WIDTH: u32 = 1440;
const HEIGHT: u32 = 960;

/// Work on the test repository: setting it up before the project exists, or seeding it after.
type RepoStep<'a> = Box<dyn FnOnce(&Path) + 'a>;

pub struct Desktop {
  pub tree: Tree,
  pub repo: PathBuf,
  /// The session's background operations, shown in the status bar.
  pub activity: Activity,
  /// The application's runners, as the root has them.
  pub runners: Runners,
  /// The keychain secret values go to: in memory, never the keychain of the user running tests.
  pub vault: Arc<MemoryVault>,
  /// The time live values are shown at.
  pub clock: Arc<TestClock>,
  runner: PathBuf,
  app: App,
  _session: AppSession,
  _runtime: Runtime,
  _dir: TempDir,
}

impl Desktop {
  /// First launch: no project yet.
  pub fn first_launch() -> Self {
    Self::start(false, no_programs(), "Kontur")
  }

  /// A project in a git repository, opened at startup.
  pub fn with_project() -> Self {
    Self::start(true, no_programs(), "Kontur")
  }

  /// A project with an exact name, for layout checks on user data.
  pub fn with_project_named(name: &str) -> Self {
    Self::start(true, no_programs(), name)
  }

  /// A project whose `gh` runs are answered by `runner`.
  pub fn with_project_and_programs(runner: Arc<dyn CommandRunner + Send + Sync>) -> Self {
    Self::start(true, Programs::with_runner(runner), "Kontur")
  }

  /// A project in the repository `init` sets up, whose runner is the program `runner`, with
  /// `seed` run on the repository once the project exists. Programs are real, as in the
  /// application: the task page reads a run's changes with git.
  pub fn with_runner(runner: PathBuf, init: impl FnOnce(&Path), seed: impl FnOnce(&Path)) -> Self {
    Self::with_runner_and_programs(runner, Programs::processes(), init, seed)
  }

  pub fn with_runner_and_programs(
    runner: PathBuf,
    programs: Programs,
    init: impl FnOnce(&Path),
    seed: impl FnOnce(&Path),
  ) -> Self {
    Self::start_with(
      Some(Box::new(init)),
      programs,
      runner,
      Some(Box::new(seed)),
      None,
      "Kontur",
    )
  }

  /// Kontur, opened at startup and filled by `seed`, and before it another project of the
  /// workspace, Personal website, in [`Desktop::other_repo`], filled by `seed_other`.
  pub fn with_other_project(seed_other: impl FnOnce(&Path), seed: impl FnOnce(&Path)) -> Self {
    let init: RepoStep<'_> = Box::new(|repo: &Path| fs::create_dir_all(repo.join(".git")).expect("create .git"));
    Self::start_with(
      Some(init),
      no_programs(),
      PathBuf::from("no-runner.exe"),
      Some(Box::new(seed)),
      Some(Box::new(seed_other)),
      "Kontur",
    )
  }

  /// A project whose repository `seed` fills once the project exists, e.g. with published
  /// resources; its screens run no programs.
  pub fn with_seeded_project(seed: impl FnOnce(&Path)) -> Self {
    let init: RepoStep<'static> = Box::new(|repo: &Path| fs::create_dir_all(repo.join(".git")).expect("create .git"));
    Self::start_with(
      Some(init),
      no_programs(),
      PathBuf::from("no-runner.exe"),
      Some(Box::new(seed)),
      None,
      "Kontur",
    )
  }

  fn start(with_project: bool, programs: Programs, project_name: &str) -> Self {
    let init: Option<RepoStep<'static>> = if with_project {
      Some(Box::new(|repo: &Path| {
        fs::create_dir_all(repo.join(".git")).expect("create .git")
      }))
    } else {
      None
    };
    // No runner program exists here; the project shows its runner as unable to start.
    Self::start_with(init, programs, PathBuf::from("no-runner.exe"), None, None, project_name)
  }

  fn start_with<'a>(
    init: Option<RepoStep<'a>>,
    programs: Programs,
    runner: PathBuf,
    seed: Option<RepoStep<'a>>,
    seed_other: Option<RepoStep<'a>>,
    project_name: &str,
  ) -> Self {
    let with_project = init.is_some();
    let dir = TempDir::new().expect("temporary directory");
    let repo = dir.path().join("kontur");
    fs::create_dir_all(&repo).expect("create the repository folder");
    match init {
      Some(init) => init(&repo),
      None => fs::create_dir_all(repo.join(".git")).expect("create .git"),
    }
    let session = AppSession::start_at(&dir.path().join("orchester.db")).expect("session starts");
    if let Some(seed_other) = seed_other {
      // Created first, so Kontur is the most recently opened project and the one shown.
      let other = dir.path().join(OTHER_REPO);
      fs::create_dir_all(other.join(".git")).expect("create the other repository");
      let new = ProjectDraft {
        name: "Personal website",
        repository: Some(&other),
      }
      .validate()
      .expect("valid draft");
      session.workspace().create(new).expect("the other project is created");
      seed_other(&other);
    }
    if with_project {
      let new = ProjectDraft {
        name: project_name,
        repository: Some(&repo),
      }
      .validate()
      .expect("valid draft");
      session.workspace().create(new).expect("project is created");
    }
    if let Some(seed) = seed {
      seed(&repo);
    }
    let runner = if runner.is_absolute() {
      runner
    } else {
      dir.path().join(runner)
    };
    Self::launch(dir, repo, session, programs, runner)
  }

  /// Mounts the application on the session's database, as `main` does.
  fn launch(dir: TempDir, repo: PathBuf, session: AppSession, programs: Programs, runner: PathBuf) -> Self {
    let runtime = Runtime::new().expect("tokio runtime");
    let mut app = App::new();
    app.set_tokio_handle(runtime.handle().clone());
    theme::install(&mut app);
    i18n::install(&app).expect("bundled locales load");
    let activity = Activity::new();
    let runners = Runners::new(
      runner.clone(),
      runtime.handle().clone(),
      activity.clone(),
      session.previous_end(),
    );
    let vault = Arc::new(MemoryVault::default());
    let clock = Arc::new(TestClock::default());
    let startup = Startup::Ready {
      workspace: session.workspace().clone(),
      lifecycle: Lifecycle::new(runners.clone(), runtime.handle().clone()),
      activity: activity.clone(),
      programs,
      vault: Vault::with(vault.clone()),
      previous_end: session.previous_end(),
      session_start: session.started_at(),
      clock: Clock::with(clock.clone()),
    };
    let mut tree = Tree::new();
    tree.resize(WIDTH, HEIGHT);
    // The harness starts no MCP server, so the status bar truthfully says it is unavailable.
    let mcp = McpStatus::Unavailable;
    tree.mount_root::<Root>(&mut app, RootProps { startup, mcp });
    Self {
      tree,
      repo,
      activity,
      runners,
      vault,
      clock,
      runner,
      app,
      _session: session,
      _runtime: runtime,
      _dir: dir,
    }
  }

  /// The application ends without recording its end, as when its process is killed. Runtime
  /// shutdown drops the runner connections. After `ended` performs any test-specific cleanup,
  /// it starts again on the same database, which reports the crash.
  pub fn crash_and_restart(self, ended: impl FnOnce()) -> Self {
    let Self {
      tree,
      repo,
      runner,
      app,
      _session: session,
      _runtime: runtime,
      _dir: dir,
      ..
    } = self;
    drop(tree);
    drop(app);
    // Ending the runtime drops the runner connections, which terminates the runners.
    runtime.shutdown_timeout(Duration::from_secs(5));
    ended();
    drop(session);
    let session = AppSession::start_at(&dir.path().join("orchester.db")).expect("session starts again");
    Self::launch(dir, repo, session, Programs::processes(), runner)
  }

  pub fn repo(&self) -> &Path {
    &self.repo
  }

  /// The repository of the other project [`Desktop::with_other_project`] creates.
  pub fn other_repo(&self) -> PathBuf {
    self._dir.path().join(OTHER_REPO)
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

  /// Lets background work (futures, the runner, its agents) go on until `done` holds, and fails
  /// the test once [`PATIENCE`] has passed without it.
  pub fn wait_until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
    let start = Instant::now();
    loop {
      self.tree.tick_futures();
      self.pass();
      if done(self) {
        return;
      }
      assert!(
        start.elapsed() < PATIENCE,
        "timed out after {PATIENCE:?} waiting for {what}"
      );
      thread::sleep(POLL_PAUSE);
    }
  }

  /// Starts the embedded MCP server on the window's tree, as `main` does, and answers `agent`'s
  /// calls between passes until it is done. Once per desktop: a tree has one server.
  pub fn with_mcp_agent<T: Send + 'static>(&mut self, agent: impl FnOnce(McpAgent) -> T + Send + 'static) -> T {
    let mcp = self.tree.enable_mcp(McpConfig::new().app_name("orchester-test"));
    let (port, token) = (mcp.port(), mcp.token().to_owned());
    assert_ne!(port, 0, "the MCP server listens");
    let agent = thread::spawn(move || agent(McpAgent::connect(port, token)));
    let start = Instant::now();
    while !agent.is_finished() {
      assert!(start.elapsed() < PATIENCE, "the MCP agent did not finish");
      if self.tree.drain_mcp_requests(&mut self.app) {
        self.pass();
      }
      thread::sleep(POLL_PAUSE);
    }
    self.tree.shutdown_mcp();
    agent.join().expect("the MCP agent ends without a panic")
  }

  /// [`Desktop::wait_until`] what the tree shows.
  pub fn settle(&mut self, what: &str, done: impl Fn(&mut Tree) -> bool) {
    self.wait_until(what, |desktop| done(&mut desktop.tree));
  }

  pub fn wait_for(&mut self, id: &str) {
    self.settle(id, |tree| tree.get_element_by_id(id).is_some());
  }

  /// Waits for `id` to be shown before it is clicked or pressed, as a person waits for what they
  /// mean to click: background work only arrives while the harness waits, so rows or dialog
  /// content read in the background would otherwise race the click. An element that is shown is
  /// not waited for, so no background work goes on before it is clicked.
  fn shown_to_click(&mut self, id: &str) {
    if self.tree.get_element_by_id(id).is_none() {
      self.settle(&format!("{id} to click"), |tree| tree.get_element_by_id(id).is_some());
    }
  }

  pub fn click(&mut self, id: &str) {
    self.shown_to_click(id);
    self.tree.get_element_by_id_mut(id).expect(id).click();
    self.pass();
  }

  /// Where `id` is laid out in the window.
  pub fn bounds(&mut self, id: &str) -> ElementRect {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// A pointer press and release in the middle of `id`, as a user clicks.
  pub fn press(&mut self, id: &str) {
    self.shown_to_click(id);
    let bounds = self.bounds(id);
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.pass();
  }

  /// The pointer moves over the middle of `id`.
  pub fn hover(&mut self, id: &str) {
    let bounds = self.bounds(id);
    self
      .tree
      .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.pass();
  }

  /// The fill painted topmost at the middle of `id` in the last pass, text and strokes aside: its
  /// own fill, or what shows through where it paints none.
  pub fn fill_at(&mut self, id: &str) -> Option<Color> {
    let bounds = self.bounds(id);
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self
      .tree
      .painted_quads()
      .into_iter()
      .filter(|quad| covers(quad, x, y))
      .filter_map(|quad| match quad.content {
        QuadContent::Rect { color, .. } if color.a() > 0 => Some(color),
        _ => None,
      })
      .last()
  }

  /// `role` as the desktop's theme resolves it.
  pub fn color(&self, role: impl Into<PaletteColor>) -> Color {
    self.app.theme().palette_color(role)
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

/// Whether `quad` paints the point (`x`, `y`): inside its frame and its clip.
fn covers(quad: &Quad, x: f32, y: f32) -> bool {
  let inside =
    |left: f32, top: f32, width: f32, height: f32| x >= left && x <= left + width && y >= top && y <= top + height;
  let clip = &quad.clip;
  inside(quad.x, quad.y, quad.width, quad.height) && (!clip.active || inside(clip.x, clip.y, clip.width, clip.height))
}

/// A keychain in memory. It keeps what it is given, so tests can check the value arrived.
#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl MemoryVault {
  /// The stored values, in no particular order.
  pub fn values(&self) -> Vec<String> {
    self.0.lock().values().cloned().collect()
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// Programs for screens that are not expected to run any: a run fails the test.
fn no_programs() -> Programs {
  struct NoPrograms;
  impl CommandRunner for NoPrograms {
    fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
      panic!("`{}` was run by a test that runs no programs", command.args().join(" "));
    }
  }
  Programs::with_runner(Arc::new(NoPrograms))
}

/// Expected [`Desktop::stops`], written with `&str`.
pub fn expected(stops: &[Option<&str>]) -> Vec<Option<String>> {
  stops.iter().map(|stop| stop.map(str::to_owned)).collect()
}
