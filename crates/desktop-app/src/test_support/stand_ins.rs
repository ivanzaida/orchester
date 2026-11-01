//! The window with the system's pickers, clipboard, CLI search and clone console replaced: a picker that hands over
//! the path a test chose, an *Attach* picker that hands over the files a test chose, a copier that keeps what was
//! copied, a search for agent CLIs in folders a test names, in none until it does, and a clone console that does what
//! a test scripts ([`ConsoleScript`]), so no native dialog or console window opens, the person's clipboard is left
//! alone and what is installed on the machine does not change what a test sees.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_exec::search_path::{SearchBasis, SearchPath};
use orchester_workspace::sharing::project::clone::{Clones, LaunchJob};
use parking_lot::Mutex;

pub use super::stand_in_console::{Console, ConsoleScript};
use super::{Data, Window};
use crate::{
  mcp::{McpKind, McpLink, McpSettings, McpStatus},
  root::{Root, RootProps},
  services::{attachment_picker::AttachmentPicker, cli_search::CliSearch, path_picker::PathPicker},
  startup::Launch,
  ui::code_box::Copier,
};

/// What the stand-in pickers hand over next, what the stand-in copier was given, where the stand-in search looks, how
/// often it was asked to read the login shell again, and what the stand-in clone console does and was given.
#[derive(Clone)]
pub struct StandIns {
  pub next: Arc<Mutex<Option<PathBuf>>>,
  pub attach: Arc<Mutex<Vec<PathBuf>>>,
  pub copied: Arc<Mutex<Vec<String>>>,
  pub searched: Arc<Mutex<(SearchPath, SearchBasis)>>,
  pub shell_reads: Arc<Mutex<u32>>,
  pub console: Console,
}

impl Default for StandIns {
  fn default() -> Self {
    Self {
      next: Arc::default(),
      attach: Arc::default(),
      copied: Arc::default(),
      searched: Arc::new(Mutex::new((SearchPath::default(), SearchBasis::Path))),
      shell_reads: Arc::default(),
      console: Console::default(),
    }
  }
}

impl StandIns {
  /// The picker hands over `path` the next times it is opened; `None` closes it without a choice.
  pub fn choose(&self, path: Option<PathBuf>) {
    *self.next.lock() = path;
  }

  /// *Attach*'s picker hands over `files` the next times it is opened; none closes it without a choice.
  pub fn attach(&self, files: Vec<PathBuf>) {
    *self.attach.lock() = files;
  }

  /// Agent CLIs are looked for in `search`, which looked at `basis`, from the next search on.
  pub fn search(&self, search: SearchPath, basis: SearchBasis) {
    *self.searched.lock() = (search, basis);
  }

  /// The clone console does as `script` says from the next clone on.
  pub fn console(&self, script: ConsoleScript) {
    self.console.script(script);
  }

  /// The clones the stand-in console was asked to run, in order.
  pub fn consoles(&self) -> Vec<LaunchJob> {
    self.console.jobs()
  }

  /// A console that holds ([`ConsoleScript::Holds`]) ends as the script after it says.
  pub fn release_console(&self) {
    self.console.release();
  }

  fn clones(&self) -> Clones {
    let console = self.console.clone();
    Clones::with_launcher(move |job| console.run(job))
  }

  fn picker(&self) -> PathPicker {
    let next = self.next.clone();
    PathPicker::new(move |_, _| {
      let chosen = next.lock().clone();
      Box::pin(async move { chosen })
    })
  }

  fn attach_picker(&self) -> AttachmentPicker {
    let next = self.attach.clone();
    AttachmentPicker::new(move |_| {
      let chosen = next.lock().clone();
      Box::pin(async move { chosen })
    })
  }

  fn copier(&self) -> Copier {
    let copied = self.copied.clone();
    Copier::other(move |text| {
      copied.lock().push(text.to_owned());
      true
    })
  }

  fn cli_search(&self) -> CliSearch {
    let (searched, shell_reads) = (self.searched.clone(), self.shell_reads.clone());
    CliSearch::new(move |read_again| {
      if read_again {
        *shell_reads.lock() += 1;
      }
      searched.lock().clone()
    })
  }
}

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  picker: PathPicker,
  attach: AttachmentPicker,
  copier: Copier,
  cli_search: CliSearch,
  clones: Clones,
}

impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for HostProps {}

/// The window's root with the stand-ins in its context.
struct Host;

impl Component for Host {
  type Props = HostProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    ctx.provide(props.picker);
    ctx.provide(props.attach);
    ctx.provide(props.copier);
    ctx.provide(props.cli_search);
    ctx.provide(props.clones);
    ctx.mount_keyed::<Root>("app", props.root)
  }
}

impl Window {
  /// The window `data` starts, with the stand-ins.
  pub fn with_stand_ins(data: Data) -> (Self, StandIns) {
    let launch = data.launch();
    Self::launched_with_stand_ins(data, launch)
  }

  /// The window `launch` starts on `data`, with the stand-ins.
  pub fn launched_with_stand_ins(data: Data, launch: Launch) -> (Self, StandIns) {
    let mut window = Self::of(data, launch);
    let stand_ins = StandIns::default();
    let props = HostProps {
      root: RootProps {
        link: window.link.clone(),
        mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
        start: window.launch.start(),
      },
      picker: stand_ins.picker(),
      attach: stand_ins.attach_picker(),
      copier: stand_ins.copier(),
      cli_search: stand_ins.cli_search(),
      clones: stand_ins.clones(),
    };
    window.tree.mount_root::<Host>(&mut window.app, props);
    window.pass();
    (window, stand_ins)
  }
}
