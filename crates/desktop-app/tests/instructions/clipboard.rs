//! A real preview button uses a clipboard owned solely by this headless fixture.
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
use orchester_core::resource::InstructionScope;
use parking_lot::Mutex;

use super::{harness::Instructions, preview::seed_preview};

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
#[test]
fn instructions_copied_receipt_resets_for_new_event_and_draft_without_rewriting_clipboard() {
  let copied = Arc::new(Mutex::new(Vec::<String>::new()));
  let sink = copied.clone();
  let clipboard = Clipboard::new(move |text| {
    sink.lock().push(text.to_owned());
    true
  });
  let mut screen = Instructions::new();
  screen.window.tree.mount_root::<Host>(
    &mut screen.window.app,
    HostProps {
      root: RootProps {
        link: screen.window.link.clone(),
        mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
        start: screen.window.launch.start(),
      },
      clipboard,
    },
  );
  screen.window.pass();
  seed_preview(&screen);
  screen.rule("Agreement", "Original {{task.title}}", InstructionScope::Project, true);
  screen.go();
  screen.click("instruction-panel-preview");
  screen.until("copy button", |s| s.exists("instruction-copy-prompt"));
  screen.click("instruction-copy-prompt");
  assert!(screen.text("instruction-copy-prompt").contains("Copied"));
  assert!(copied.lock()[0].contains("Original Add retry"));
  screen.choose("instruction-preview-event", "Agent resume");
  assert!(!screen.text("instruction-copy-prompt").contains("Copied"));
  assert_eq!(copied.lock().len(), 1, "a new context only clears the receipt");
  screen.choose("instruction-preview-event", "Agent start");
  screen.click("instruction-copy-prompt");
  assert!(screen.text("instruction-copy-prompt").contains("Copied"));
  screen.set("instruction-prompt", "New {{task.title}}");
  assert!(!screen.text("instruction-copy-prompt").contains("Copied"));
  assert_eq!(copied.lock().len(), 2);
  assert!(copied.lock()[1].contains("Original Add retry"));
  screen.click("instruction-copy-prompt");
  assert!(copied.lock()[2].contains("New Add retry"));
}
