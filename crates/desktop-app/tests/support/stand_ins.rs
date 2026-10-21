//! The window with the system's pickers and clipboard replaced: a picker that hands over the path a test chose, an
//! *Attach* picker that hands over the files a test chose, and a copier that keeps what was copied, so no native
//! dialog opens and the person's clipboard is left alone.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_app::{
  mcp::{McpKind, McpLink, McpSettings, McpStatus},
  root::{Root, RootProps},
  services::{attachment_picker::AttachmentPicker, path_picker::PathPicker},
  startup::Launch,
  ui::code_box::Copier,
};
use parking_lot::Mutex;

use super::{Data, Window};

/// What the stand-in pickers hand over next, and what the stand-in copier was given.
#[derive(Clone, Default)]
pub struct StandIns {
  pub next: Arc<Mutex<Option<PathBuf>>>,
  pub attach: Arc<Mutex<Vec<PathBuf>>>,
  pub copied: Arc<Mutex<Vec<String>>>,
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
}

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  picker: PathPicker,
  attach: AttachmentPicker,
  copier: Copier,
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
    };
    window.tree.mount_root::<Host>(&mut window.app, props);
    window.pass();
    (window, stand_ins)
  }
}
