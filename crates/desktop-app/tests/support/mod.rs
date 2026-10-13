//! The app's window headless at the design's 1440 × 900, on a session of a data folder of its own: the
//! real root with the theme and the bundled locales, the project Kontur (KON) registered in a temporary
//! git repository, and runners whose program does not exist, so a project's runner fails to start
//! (`MissingExecutable`) without starting anything.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{
  fs,
  path::{Path, PathBuf},
  thread,
  time::{Duration, Instant},
};

use lurq::{
  app::{App, Tree},
  node::element::ElementRef,
};
use orchester_app::{
  i18n,
  mcp::{McpKind, McpLink, McpSettings, McpStatus, WindowLink},
  navigation::Navigation,
  project::Projects,
  root::{Root, RootProps},
  startup::Launch,
  theme::{self, Mode},
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

/// Kontur's id, as the routes of the tests name it.
pub const PROJECT: &str = "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b";

pub fn project_id() -> ProjectId {
  PROJECT.parse().expect("a project id")
}

/// A data folder of its own, the runtime its sessions' services run on, and its repositories.
pub struct Data {
  pub dir: TempDir,
  pub runtime: Runtime,
}

impl Data {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-app-tests");
    fs::create_dir_all(&scratch).expect("worktree scratch");
    Self {
      dir: tempfile::tempdir_in(scratch).expect("private worktree directory"),
      runtime: Runtime::new().expect("tokio runtime"),
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

  /// Starts a session of the data folder. Its runner program does not exist.
  pub fn launch(&self) -> Launch {
    let runner = self.dir.path().join("no-runner").join("orchester-runner");
    Launch::begin_at(&self.database(), runner, self.runtime.handle(), None)
  }

  /// Kontur (KON) with the id [`PROJECT`] in the repository `repo-kontur`, set up through `workspace`.
  pub fn add_kontur(&self, workspace: &Workspace) -> OpenProject {
    add_project(workspace, (project_id(), "KON", "Kontur"), &self.repo("repo-kontur"))
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
  pub fn of_start(data: Data, launch: Launch, start: orchester_app::root::Start) -> Self {
    let mut app = App::new();
    app.set_tokio_handle(data.runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
<<<<<<< HEAD
        mcp: McpStatus::Unavailable,
        start,
=======
        mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
        start: launch.start(),
>>>>>>> 8754a60874f27100ec9bc2d4df91f0d70cca074c
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
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
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
