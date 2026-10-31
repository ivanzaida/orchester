//! The app's window headless at the design's 1440 × 900, on a session of a data folder of its own, for the app's tests
//! here and the end-to-end tests of the private platform repository, which build this crate by git: the real root
//! with the theme and the bundled locales, the project Kontur (KON) registered in a temporary git repository, and
//! runners whose program does not exist, so a project's runner fails to start (`MissingExecutable`) without starting
//! anything. The session is signed in to a stand-in Orchester Cloud with a keychain in memory ([`signed_in`]), past the
//! sign-in gate; [`Data::launch_signed_out`] stops at the gate. Never part of the app's program: only the
//! `test-support` feature builds it.

pub mod signed_in;
pub mod stand_ins;

use std::{
  env, fs,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_workspace::{
  project_setup::NewProject,
  workspace::{OpenProject, Workspace},
};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{McpKind, McpLink, McpSettings, McpStatus, WindowLink},
  navigation::Navigation,
  project::Projects,
  root::{Root, RootProps, Start},
  startup::Launch,
  theme::{self, Mode},
};

/// Kontur's id, as the routes of the tests name it.
pub const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

pub fn project_id() -> ProjectId {
  PROJECT.parse().expect("a project id")
}

/// A data folder of its own, the runtime its sessions' services run on, its repositories, and the stand-in cloud its
/// sessions sign in to.
pub struct Data {
  pub dir: TempDir,
  pub runtime: Runtime,
  pub cloud: signed_in::account_cloud::AccountCloud,
}

impl Data {
  /// A data folder in `dir`, its sessions' services on `runtime`.
  pub fn at(dir: TempDir, runtime: Runtime) -> Self {
    Self {
      dir,
      runtime,
      cloud: signed_in::account_cloud::AccountCloud::start(),
    }
  }

  pub fn new() -> Self {
    // The workspace's folder named without `..`, as the seed and the app name the data folder: the workspace of the
    // package `cargo test` runs the tests of, or of this crate for a test program started directly.
    let package =
      env::var_os("CARGO_MANIFEST_DIR").map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
    let workspace = package
      .ancestors()
      .nth(2)
      .expect("the crate lies two folders inside the workspace");
    let scratch = workspace.join(".tmp").join("orc-24-app-tests");
    fs::create_dir_all(&scratch).expect("worktree scratch");
    Self {
      dir: tempfile::tempdir_in(scratch).expect("private worktree directory"),
      runtime: Runtime::new().expect("tokio runtime"),
      cloud: signed_in::account_cloud::AccountCloud::start(),
    }
  }

  pub fn database(&self) -> PathBuf {
    self.dir.path().join("orchester.db")
  }

  /// A git repository `name` in the data folder, made the first time it is asked for.
  pub fn repo(&self, name: &str) -> PathBuf {
    let repo = self.dir.path().join(name);
    fs::create_dir_all(repo.join(".git")).expect("create .git");
    repo
  }

  /// Starts a session of the data folder, signed in to the stand-in cloud. Its runner program does not exist.
  pub fn launch(&self) -> Launch {
    self.launch_with(self.no_runner())
  }

  /// [`Self::launch`] with the runner program `runner`.
  pub fn launch_with(&self, runner: PathBuf) -> Launch {
    signed_in::launch(&self.database(), runner, self.runtime.handle(), &self.cloud)
  }

  /// A session of the data folder that is not signed in: the window shows the sign-in gate.
  pub fn launch_signed_out(&self) -> Launch {
    signed_in::launch_signed_out(&self.database(), self.no_runner(), self.runtime.handle())
  }

  /// A runner program that does not exist.
  pub fn no_runner(&self) -> PathBuf {
    self.dir.path().join("no-runner").join("orchester-runner")
  }

  /// Kontur (KON) with the id [`PROJECT`] in the repository `repo-kontur`, set up through `workspace`.
  pub fn add_kontur(&self, workspace: &Workspace) -> OpenProject {
    add_project(workspace, (project_id(), "KON", "Kontur"), &self.repo("repo-kontur"))
  }
}

impl Default for Data {
  fn default() -> Self {
    Self::new()
  }
}

/// The project `(id, key, name)` set up in `repo` and added to `workspace`.
pub fn add_project(workspace: &Workspace, (id, key, name): (ProjectId, &str, &str), repo: &Path) -> OpenProject {
  let project = Project::new(
    id,
    ProjectKey::new(key).expect("a project key"),
    Name::new(name).expect("a project name"),
  );
  workspace
    .create(NewProject {
      project,
      repo_root: repo.to_path_buf(),
    })
    .expect("the project is set up")
}

pub struct Window {
  pub app: App,
  pub tree: Tree,
  pub link: WindowLink,
  pub launch: Launch,
  pub data: Data,
}

impl Window {
  /// The window of a new data folder, at All projects, with Kontur in the list.
  pub fn open() -> Self {
    let data = Data::new();
    let launch = data.launch();
    data.add_kontur(launch.workspace().expect("the session started"));
    Self::of(data, launch)
  }

  /// The window `launch` opens on `data`.
  pub fn of(data: Data, launch: Launch) -> Self {
    let start = launch.start();
    Self::of_start(data, launch, start)
  }
  pub fn of_start(data: Data, launch: Launch, start: Start) -> Self {
    let mut app = App::new();
    app.set_tokio_handle(data.runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    // The window's colors, kept in the session's database as the app's own window keeps them.
    link.attach_appearance(Appearance::new(
      ThemeSetting::Dark,
      Arc::default(),
      launch.workspace().cloned(),
    ));
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
        mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
        start,
      },
    );
    tree.pass_headless(&mut app);
    Self {
      app,
      tree,
      link,
      launch,
      data,
    }
  }

  pub fn navigation(&self) -> Navigation {
    self.link.navigation().expect("the window attached its navigation")
  }

  pub fn projects(&self) -> Projects {
    self.link.projects().expect("the window attached its projects")
  }

  /// Lays the window out again after a change.
  pub fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  /// Delivers what the window's futures finished, and lays the window out, until `done` holds; panics
  /// with `what` after a few seconds.
  pub fn until(&mut self, what: &str, done: impl FnMut(&mut Self) -> bool) {
    self.until_within(Duration::from_secs(10), what, done);
  }

  /// [`Self::until`] with a deadline of `wait`, for work that takes longer than drawing, such as a git merge on a
  /// loaded machine.
  pub fn until_within(&mut self, wait: Duration, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = Instant::now() + wait;
    loop {
      self.tree.tick_futures();
      self.pass();
      if done(self) {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      thread::sleep(Duration::from_millis(5));
    }
  }

  /// Presses the middle of the element `id` with the pointer.
  pub fn press(&mut self, id: &str) {
    let (x, y) = self.middle(id);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, lurq::app::events::MouseButton::Left);
    self.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
    self.pass();
  }

  /// Moves the pointer onto the middle of the element `id`, for what shows only under it.
  pub fn hover(&mut self, id: &str) {
    let (x, y) = self.middle(id);
    self.tree.mouse_move(x, y);
    self.pass();
  }

  fn middle(&mut self, id: &str) -> (f32, f32) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
  }

  /// Types `value` into the text input `id`, in place of what it held.
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(value);
    self.pass();
  }

  /// The texts inside the element `id` on one line; panics when no element has the id.
  pub fn text(&mut self, id: &str) -> String {
    self.texts(id).unwrap_or_else(|| panic!("{id} is shown")).join(" ")
  }

  /// The texts inside the element `id`, icon glyphs left out; `None` when no element has the id.
  pub fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}
