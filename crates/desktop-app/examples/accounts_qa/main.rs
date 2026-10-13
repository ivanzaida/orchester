//! Settings › Accounts and where accounts show (`docs/design/agent-accounts.md`, layer `hgOl2`), for design
//! verification: `cargo build -p orchester-runner` once, then
//! `cargo run -p orchester-app --example accounts_qa -- <state> [account]`, `state` one of [`State::ALL`] and
//! `account` an account's name (`Work`, `Personal`, `Old`, `Lab`, `Client`, `Freelance`, `Archive`) or
//! `claude-default` / `codex-default`, to show that account instead.
//!
//! Each start seeds a new folder under `.tmp/qa/accounts/` (removed when the window closes): the seed's
//! `task-states` project and accounts in each state ([`data`]). The window runs in a child process whose `PATH` starts
//! with stand-in CLIs ([`stand_ins`]) and leaves out every folder holding a real `claude` or `codex`, with
//! `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, the home folder and the folder for new accounts in the QA folder: no real CLI is
//! asked and no real sign-in is read. The verifier connects to the window's QA server through the discovery file in
//! `.tmp/qa/accounts/mcp/` (`tools/lurq-mcp-client.py --discovery .tmp/qa/accounts/mcp`). Settings › Accounts takes a
//! person's input only, so each of its states opens directly; outside Settings the window can be driven.

#[path = "../seed/kontur/mod.rs"]
mod kontur;

mod accounts;
mod data;
mod stand_ins;

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
  root::{Root, RootProps, Start},
  routes::{AccountsPanel, AccountsView, ProjectPage, Route, TaskTab, TaskView},
  startup::Launch,
  theme::{self, Mode},
  ui::loader,
};
use orchester_core::{account::AccountCli, id::AgentAccountId};
use orchester_workspace::{accounts::trash::FolderTrash, workspace::Workspace};
use tokio::runtime::Runtime;

/// Set in the window's process: the QA folder, the application database, the project and the stopped task.
const CHILD: &str = "ORCHESTER_ACCOUNTS_QA_ROOT";
const DATABASE: &str = "ORCHESTER_ACCOUNTS_QA_DATABASE";
const PROJECT: &str = "ORCHESTER_ACCOUNTS_QA_PROJECT";
const TASK: &str = "ORCHESTER_ACCOUNTS_QA_TASK";

/// Where the window opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
  /// `t098v`: Work, a signed-in Claude Code account, with its details.
  SignedIn,
  /// `IOyJl`: adding an account, its name and folder.
  Add,
  /// `ohqHW`, `GE7a3`: the sign-in step of Freelance, not signed in (another account's with `account`).
  SignIn,
  /// `HzJSR`: Work's *Used by*.
  UsedBy,
  /// `HzJSR`: removing Work in place, its connection to move.
  Remove,
  /// `yEJPZ`: renaming Work.
  Rename,
  /// `E96CI`: every account's state in the list; `account` picks the details.
  States,
  /// `E96CI` *CLI not found*: no `codex` on the `PATH`.
  CliMissing,
  /// `EwAhH`, `XPjSc`: the task stopped at Default's plan limit, the status bar's plan limits per account; from
  /// there the window can be driven to Connections (`WaKJN`, `M42rM`), agents and pipelines.
  Task,
}

impl State {
  const ALL: [(&'static str, Self); 9] = [
    ("signed-in", Self::SignedIn),
    ("add", Self::Add),
    ("sign-in", Self::SignIn),
    ("used-by", Self::UsedBy),
    ("remove", Self::Remove),
    ("rename", Self::Rename),
    ("states", Self::States),
    ("cli-missing", Self::CliMissing),
    ("task", Self::Task),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }

  /// The account shown when none is named, and the panel.
  const fn shows(self) -> (&'static str, AccountsPanel) {
    match self {
      Self::SignedIn | Self::States => ("Work", AccountsPanel::Account),
      Self::Add => ("", AccountsPanel::Add),
      Self::SignIn => ("Freelance", AccountsPanel::SignIn),
      Self::UsedBy => ("Work", AccountsPanel::UsedBy),
      Self::Remove => ("Work", AccountsPanel::Remove),
      Self::Rename => ("Work", AccountsPanel::Rename),
      Self::CliMissing | Self::Task => ("codex-default", AccountsPanel::Account),
    }
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let mut args = env::args().skip(1);
  let name = args.next().unwrap_or_else(|| "signed-in".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  let account = args.next();
  match env::var_os(CHILD) {
    Some(_) => window(state, account.as_deref()),
    None => start(state),
  }
}

/// Seeds a new QA folder and runs the window in a child process set up with the stand-ins; returns once it closed.
fn start(state: State) -> Result<(), Box<dyn Error>> {
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let root = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let folders = Folders {
    root: root.path().to_path_buf(),
  };
  let prepared = data::prepare(&folders, state == State::CliMissing)?;
  let mut path = vec![folders.path()];
  path.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()).filter(|folder| !holds_a_cli(folder)));
  let mut child = Command::new(env::current_exe()?)
    .args(env::args().skip(1))
    .env("PATH", env::join_paths(path)?)
    .env("CLAUDE_CONFIG_DIR", folders.claude_default())
    .env("CODEX_HOME", folders.codex_default())
    .env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, folders.home())
    .env("ORCHESTER_ACCOUNTS_DIR", folders.accounts())
    .env(CHILD, root.path())
    .env(DATABASE, &prepared.database)
    .env(PROJECT, prepared.project.to_string())
    .env(TASK, &prepared.task)
    .spawn()?;
  writeln!(
    std::io::stdout().lock(),
    "pid {}\nmcp {}\ndata {}\nseeded {}",
    child.id(),
    scratch.join("mcp").display(),
    root.path().display(),
    prepared.shown
  )?;
  let status = child.wait()?;
  if !status.success() {
    return Err(format!("the QA window ended with {status}").into());
  }
  Ok(())
}

/// `.tmp/qa/accounts/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("accounts"))
    .ok_or("the crate is two folders into the checkout")
}

/// Whether `folder` holds a real `claude` or `codex`, which the window must not find.
fn holds_a_cli(folder: &Path) -> bool {
  ["claude", "codex"].iter().any(|program| {
    ["", ".exe", ".cmd", ".bat", ".ps1"]
      .iter()
      .any(|extension| folder.join(format!("{program}{extension}")).is_file())
  })
}

/// The window on the seeded data at `state`; returns once it closed.
fn window(state: State, account: Option<&str>) -> Result<(), Box<dyn Error>> {
  let variable = |name: &str| env::var_os(name).ok_or_else(|| format!("{name} is set by the QA start"));
  let database = PathBuf::from(variable(DATABASE)?);
  let project = variable(PROJECT)?.to_string_lossy().parse()?;
  let task = variable(TASK)?.to_string_lossy().into_owned();
  let runtime = Runtime::new()?;
  let runner = env::current_exe()?
    .parent()
    .and_then(Path::parent)
    .ok_or("the example's folder is in the target folder")?
    .join(format!("orchester-runner{}", env::consts::EXE_SUFFIX));
  let launch = Launch::begin_at(&database, runner, runtime.handle(), None);
  let workspace = launch.workspace().ok_or("the QA session did not start")?.clone();
  // Removed accounts' folders go to the QA folder's own trash, never the person's Recycle Bin.
  let root = PathBuf::from(variable(CHILD)?);
  workspace.use_trash(std::sync::Arc::new(FolderTrash::new(&root.join("trash"))));
  let route = match state {
    State::Task => Route::Project(
      project,
      ProjectPage::Task(task.parse()?, TaskView::of(TaskTab::Summary)),
    ),
    _ => {
      let (shown, panel) = state.shows();
      let account = match (state, account) {
        (State::Add, None) => None,
        (_, named) => Some(account_named(&workspace, named.unwrap_or(shown))?),
      };
      Route::Accounts(AccountsView { account, panel })
    }
  };
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = route;
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
  let discovery = qa_folder()?.join("mcp");
  let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(discovery),
    (config, settings),
    &link.input_gate(),
  );
  tree.mount_root::<Root>(
    &mut app,
    RootProps {
      link,
      mcp: McpLink::new(&server),
      start,
    },
  );
  WinitWindow::new(app, tree)
    .with_title("Orchester accounts QA")
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

/// The account called `name`, or a CLI's Default (`claude-default`, `codex-default`).
fn account_named(workspace: &Workspace, name: &str) -> Result<AgentAccountId, Box<dyn Error>> {
  match name {
    "claude-default" => return Ok(AccountCli::ClaudeCode.default_account()),
    "codex-default" => return Ok(AccountCli::Codex.default_account()),
    _ => {}
  }
  workspace
    .agent_accounts()?
    .into_iter()
    .find(|account| {
      account
        .name()
        .is_some_and(|known| known.as_str().eq_ignore_ascii_case(name))
    })
    .map(|account| account.id())
    .ok_or_else(|| format!("no account {name}").into())
}
