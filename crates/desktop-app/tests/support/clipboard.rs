//! A window's own clipboard, so a test copies without touching the person's: the window's root mounted again under a
//! host that provides a [`Clipboard`] keeping what was copied, or refusing it.

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

use super::Window;

/// What the window copied, in order.
pub type Copied = Arc<Mutex<Vec<String>>>;

#[derive(Clone)]
struct HostProps {
  root: RootProps,
  clipboard: Clipboard,
}

impl PartialEq for HostProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.clipboard == other.clipboard
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

/// Mounts `window`'s root again with a clipboard of its own, which takes what is copied when `takes`; what it took.
pub fn own_clipboard(window: &mut Window, takes: bool) -> Copied {
  let copied: Copied = Arc::default();
  let sink = copied.clone();
  let clipboard = Clipboard::new(move |text| {
    if takes {
      sink.lock().push(text.to_owned());
    }
    takes
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
