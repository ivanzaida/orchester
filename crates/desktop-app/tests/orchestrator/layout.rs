//! The chat and its settings share one head (`screens::orchestrator::page`): at 1440 × 900 and 960 wide, switching
//! between them moves neither the breadcrumb, the title nor the content's left edge.
use orchester_app::routes::OrchestratorChat;

use super::harness::{Orchestrator, Setup};

/// Where a page puts its breadcrumb and title (x, y), the line under its title and its content (x).
#[derive(Debug)]
struct Placed {
  crumbs: (f32, f32),
  title: (f32, f32),
  line: f32,
  content: f32,
}

fn at(screen: &mut Orchestrator, id: &str) -> (f32, f32) {
  let bounds = screen.bounds(id);
  (bounds.x, bounds.y)
}

fn chat(screen: &mut Orchestrator) -> Placed {
  screen.go(OrchestratorChat::New);
  Placed {
    crumbs: at(screen, "orchestrator-crumbs"),
    title: at(screen, "orchestrator-title"),
    line: at(screen, "orchestrator-coordinator").0,
    content: at(screen, "orchestrator-start").0,
  }
}

fn settings(screen: &mut Orchestrator) -> Placed {
  screen.settings();
  Placed {
    crumbs: at(screen, "orchestrator-settings-crumbs"),
    title: at(screen, "orchestrator-settings-title"),
    line: at(screen, "orchestrator-settings-summary").0,
    content: at(screen, "orchestrator-settings-form").0,
  }
}

fn same(a: f32, b: f32) -> bool {
  (a - b).abs() < 0.5
}

#[test]
fn orchestrator_chat_and_settings_share_the_head() {
  let mut screen = Orchestrator::new(Setup::Ready);
  for (width, height) in [(1440, 900), (960, 900)] {
    screen.window.tree.resize(width, height);
    screen.window.pass();
    let chat = chat(&mut screen);
    let settings = settings(&mut screen);
    let shown = format!("at {width} wide: chat {chat:?}, settings {settings:?}");
    assert!(
      same(chat.crumbs.0, settings.crumbs.0) && same(chat.crumbs.1, settings.crumbs.1),
      "breadcrumb, {shown}"
    );
    assert!(
      same(chat.title.0, settings.title.0) && same(chat.title.1, settings.title.1),
      "title, {shown}"
    );
    let edge = settings.title.0;
    assert!(same(chat.crumbs.0, edge), "breadcrumb over the title's start, {shown}");
    assert!(
      same(chat.line, edge) && same(settings.line, edge),
      "the line under the title, {shown}"
    );
    assert!(
      same(settings.content, edge),
      "the form under the title's start, {shown}"
    );
    // The chat's column is centred and keeps its own width: from the title's start when the window leaves it no more
    // room, never left of it.
    assert!(chat.content > edge - 0.5, "the chat's column, {shown}");
    if width == 960 {
      assert!(same(chat.content, edge), "the chat's column fills its room, {shown}");
    }
  }
}
