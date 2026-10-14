//! The welcome, All projects and Add project (layer `Welcome & projects` `CGlZY`), for design verification:
//! `cargo build -p orchester-runner` once, then `cargo run -p orchester-app --example projects_qa -- <state>`,
//! `state` one of [`State::ALL`].
//!
//! Each start seeds a new folder under `.tmp/qa/projects/` ([`data`]): an application database (empty for the
//! welcome, else the seed's `task-states` Kontur and projects in each state of the list) and the folders Add
//! project is shown with. The window runs in a child process whose `PATH` starts with a stand-in `gh` (built with
//! `rustc` from `gh.rs`) answering as the state's GitHub state, and leaves out every folder holding a real `gh`, so
//! no real GitHub sign-in is read. The system's pickers are replaced: a folder picker hands over the state's folder
//! and the file picker (*Choose program…*) the stand-in `gh`; Copy writes to `copied.txt` in the QA folder, never the
//! clipboard. The verifier connects through the discovery file in `.tmp/qa/projects/mcp/`
//! (`tools/lurq-mcp-client.py --discovery .tmp/qa/projects/mcp`).

#[path = "../seed/kontur/mod.rs"]
mod kontur;

mod data;
mod host;

use std::{
  env,
  error::Error,
  io::Write as _,
  path::{Path, PathBuf},
  process::Command,
  sync::{Arc, OnceLock},
};

use data::Folders;
use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{RootProps, Start},
  routes::{Route, RouteFolder},
  startup::Launch,
  theme::{self, Mode},
  ui::loader,
};
use orchester_workspace::workspace::Workspace;
use tokio::runtime::Runtime;

/// Set in the window's process: the QA folder and the application database.
const CHILD: &str = "ORCHESTER_PROJECTS_QA_ROOT";
const DATABASE: &str = "ORCHESTER_PROJECTS_QA_DATABASE";

/// Where the window opens, with what.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
  /// `UQZRa`: a first launch.
  Welcome,
  /// `S5HDKf`, `kwpO2`: the list in every state; the × of a row asks to remove it.
  List,
  /// `kwpO2` *Empty list*: every project was removed.
  Empty,
  /// `kwpO2` *The list can't be read*.
  Failed,
  /// `kwpO2` *Open project folder… failed*: the picker hands over a repository that keeps no project.
  OpenFolder,
  /// `W5PIR`: no folder yet; *Choose folder…* hands over a repository with a GitHub remote.
  Setup,
  /// A folder already chosen, with its GitHub state (`kkGWj`, `vJTcd` and the states of `gh5wV`).
  Detected(&'static str),
}

impl State {
  const ALL: [(&'static str, Self); 17] = [
    ("welcome", Self::Welcome),
    ("list", Self::List),
    ("empty", Self::Empty),
    ("failed", Self::Failed),
    ("open-folder", Self::OpenFolder),
    ("setup", Self::Setup),
    ("setup-checking", Self::Detected("checking")),
    ("setup-detected", Self::Detected("detected")),
    ("setup-local-only", Self::Detected("local-only")),
    ("setup-not-github", Self::Detected("not-github")),
    ("setup-gh-not-found", Self::Detected("gh-not-found")),
    ("setup-not-signed-in", Self::Detected("not-signed-in")),
    ("setup-wrong-account", Self::Detected("wrong-account")),
    ("setup-no-access", Self::Detected("no-access")),
    ("setup-unrecognized", Self::Detected("unrecognized")),
    ("setup-missing-scope", Self::Detected("missing-scope")),
    ("setup-not-repository", Self::Detected("not-repository")),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }

  /// How the stand-in `gh` answers.
  const fn github(self) -> &'static str {
    match self {
      Self::Detected(github) => github,
      _ => "detected",
    }
  }

  /// The folder the folder picker hands over and, once chosen, Add project shows.
  fn folder(self) -> &'static str {
    match self {
      Self::Welcome | Self::List | Self::Empty | Self::Failed | Self::OpenFolder => "scratch",
      Self::Detected("local-only") => "local",
      Self::Detected("not-github") => "other-host",
      Self::Detected("not-repository") => "kontur-notes",
      Self::Setup | Self::Detected(_) => "kontur",
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let name = env::args().nth(1).unwrap_or_else(|| "list".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  match env::var_os(CHILD) {
    Some(root) => window(state, &Folders { root: root.into() }),
    None => start(state),
  }
}

/// Seeds a new QA folder and runs the window in a child process set up with the stand-in `gh`; returns once it
/// closed.
fn start(state: State) -> Result<(), Box<dyn Error>> {
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let root = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let folders = Folders {
    root: root.path().to_path_buf(),
  };
  data::stand_in(&folders, state.github())?;
  data::folders(&folders)?;
  let seeded = match state {
    State::Welcome => data::empty(&folders)?,
    _ => data::projects(&folders)?,
  };
  let mut path = Vec::new();
  if state.github() != "gh-not-found" {
    path.push(folders.path());
  }
  path.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()).filter(|folder| !holds_gh(folder)));
  let mut child = Command::new(env::current_exe()?)
    .args(env::args().skip(1))
    .env("PATH", env::join_paths(path)?)
    .env(CHILD, root.path())
    .env(DATABASE, &seeded.database)
    .spawn()?;
  writeln!(
    std::io::stdout().lock(),
    "pid {}\nmcp {}\ndata {}\nseeded {}",
    child.id(),
    scratch.join("mcp").display(),
    root.path().display(),
    seeded.shown,
  )?;
  let status = child.wait()?;
  if !status.success() {
    return Err(format!("the QA window ended with {status}").into());
  }
  Ok(())
}

/// `.tmp/qa/projects/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("projects"))
    .ok_or("the crate is two folders into the checkout")
}

/// Whether `folder` holds a real `gh`, which the window must not find.
fn holds_gh(folder: &Path) -> bool {
  ["gh", "gh.exe", "gh.cmd", "gh.bat"]
    .iter()
    .any(|program| folder.join(program).is_file())
}

/// The window on the seeded data at `state`; returns once it closed.
fn window(state: State, folders: &Folders) -> Result<(), Box<dyn Error>> {
  let database = PathBuf::from(env::var_os(DATABASE).ok_or("the QA start sets the database")?);
  let runtime = Runtime::new()?;
  let runner = env::current_exe()?
    .parent()
    .and_then(Path::parent)
    .ok_or("the example's folder is in the target folder")?
    .join(format!("orchester-runner{}", env::consts::EXE_SUFFIX));
  let launch = Launch::begin_at(&database, runner, runtime.handle(), None);
  let workspace = launch.workspace().ok_or("the QA session did not start")?.clone();
  prepare(state, &workspace, &database)?;
  let route = match state {
    State::Setup => Route::AddProject(None),
    State::Detected(_) => Route::AddProject(RouteFolder::new(&folders.folder(state.folder()))),
    _ => Route::AllProjects,
  };
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = route;
      ready.landing.project = None;
      Start::Ready(ready)
    }
    problem @ Start::Problem(_) => problem,
  };
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let settings = McpSettings::with_window_control(true);
  let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(qa_folder()?.join("mcp")),
    (config, settings),
    &link.input_gate(),
  );
  let root = RootProps {
    link,
    mcp: McpLink::new(&server),
    start,
  };
  tree.mount_root::<host::Host>(
    &mut app,
    host::props(
      root,
      folders.folder(state.folder()),
      folders.program(),
      folders.root.join("copied.txt"),
    ),
  );
  WinitWindow::new(app, tree)
    .with_title("Orchester projects QA")
    .with_icon(brand::window_icon()?)
    .with_size(1440, 900)
    .with_min_size(960, 640)
    .with_start_without_focus(true)
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded)
    .run();
  server.shutdown();
  launch.end();
  Ok(())
}

/// What the state needs of the session's data before the window opens: every project removed from the list
/// after it was listed (*Empty*), or a list that cannot be read (*Failed*).
fn prepare(state: State, workspace: &Workspace, database: &Path) -> Result<(), Box<dyn Error>> {
  match state {
    State::Empty => {
      workspace.listing()?;
      for project in workspace.projects()? {
        workspace.forget(project.id)?;
      }
    }
    State::Failed => {
      rusqlite::Connection::open(database)?.execute("DROP TABLE projects", [])?;
    }
    _ => {}
  }
  Ok(())
}
