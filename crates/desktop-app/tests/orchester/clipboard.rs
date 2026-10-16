//! The window with a clipboard the test owns, so a test sees what was copied and the system's clipboard is never
//! touched.

use std::sync::Arc;

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
  ui::clipboard::Clipboard,
};
use parking_lot::Mutex;

use super::support::Window;

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  clipboard: Clipboard,
}

impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
  }
}

impl DevtoolsInspectable for HostProps {}

struct Host;

impl Component for Host {
  type Props = HostProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    ctx.provide(props.clipboard);
    ctx.mount_keyed::<Root>("app", props.root)
  }
}

/// Mounts the window's app again under a clipboard of its own; answers what was copied to it.
pub fn own_clipboard(window: &mut Window) -> Arc<Mutex<Vec<String>>> {
  let copied = Arc::new(Mutex::new(Vec::new()));
  let sink = copied.clone();
  let clipboard = Clipboard::new(move |text| {
    sink.lock().push(text.to_owned());
    true
  });
  let root = RootProps {
    link: window.link.clone(),
    mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
    start: window.launch.start(),
  };
  window
    .tree
    .mount_root::<Host>(&mut window.app, HostProps { root, clipboard });
  window.pass();
  copied
}
