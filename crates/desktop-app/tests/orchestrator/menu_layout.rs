//! The @ and slash menus open over the Orchestrator's composer (ORC-891): opening, filtering and closing either one
//! moves neither the composer's box nor the conversation, also with a file attached and in a long conversation at its
//! end.

use lurq::app::events::{ClipboardContent, ClipboardImage};
use orchester_app::routes::OrchestratorChat;

use super::{
  harness::{Orchestrator, Setup},
  scroll::{long_conversation, newest_top, settle, turns},
};

const INPUT: &str = "orchestrator-input";
const MENTIONS: &str = "orchestrator-composer-mention-menu";
const SLASH: &str = "orchestrator-composer-slash-menu";
/// What must not move while a menu opens and closes: the composer's box, and the conversation's view or a new
/// conversation's start, whichever shows.
const STILL: [&str; 3] = ["orchestrator-composer", "orchestrator-scroll", "orchestrator-start"];

/// The field with the focus, holding `text` with the caret at its end.
fn write(screen: &mut Orchestrator, text: &str) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, text);
  settle(screen);
}

/// An element's x, y, width and height, by its id.
type Placed = (&'static str, [f32; 4]);

/// Each of [`STILL`] that shows, with its x, y, width and height as last laid out.
fn still(screen: &mut Orchestrator) -> Vec<Placed> {
  let shown: Vec<&'static str> = STILL.into_iter().filter(|id| screen.exists(id)).collect();
  shown
    .into_iter()
    .map(|id| {
      let bounds = screen.bounds(id);
      (id, [bounds.x, bounds.y, bounds.width, bounds.height])
    })
    .collect()
}

/// A pasted image in the composer's tray.
fn attach(screen: &mut Orchestrator) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  let image = ClipboardImage::from_rgba(1, 1, vec![0, 0, 0, 255]).expect("a pixel");
  assert!(screen.window.tree.paste(ClipboardContent::image(image)));
  screen.until("the pasted image's chip", |screen| {
    screen
      .text("orchestrator-composer-attachments")
      .contains("Pasted image.png")
  });
  settle(screen);
}

/// Types `opening` (which opens `menu`), then `filtered`, then closes it with Esc, and checks that the box and the
/// conversation stay where they were with the field holding `before`, and the newest turn too.
fn holds_still(screen: &mut Orchestrator, (before, opening, filtered): (&str, &str, &str), menu: &str) {
  write(screen, before);
  let closed = still(screen);
  let newest = newest_top_if_any(screen);
  write(screen, opening);
  screen.until("the menu", |screen| screen.exists(menu));
  settle(screen);
  assert_eq!(still(screen), closed, "opening {menu} moves nothing");
  assert_eq!(newest_top_if_any(screen), newest, "the turns stay put as {menu} opens");
  write(screen, filtered);
  assert_eq!(still(screen), closed, "filtering {menu} moves nothing");
  screen.key(("Escape", "Escape"), false);
  screen.until("the menu to close", |screen| !screen.exists(menu));
  settle(screen);
  assert_eq!(still(screen), closed, "closing {menu} moves nothing");
  assert_eq!(newest_top_if_any(screen), newest, "the turns stay put as {menu} closes");
}

/// Where the newest turn's top sits in the turns' view, when there is a turn.
fn newest_top_if_any(screen: &mut Orchestrator) -> Option<f32> {
  (!turns(screen).is_empty()).then(|| newest_top(screen))
}

#[test]
fn orchestrator_menu_layout_the_menus_open_without_moving_the_composer() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  holds_still(&mut screen, ("Compare", "Compare @", "Compare @zz"), MENTIONS);
  holds_still(&mut screen, ("", "/", "/hist"), SLASH);
}

#[test]
fn orchestrator_menu_layout_holds_with_a_file_attached() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  attach(&mut screen);
  holds_still(&mut screen, ("Compare", "Compare @", "Compare @zz"), MENTIONS);
  holds_still(&mut screen, ("", "/", "/hist"), SLASH);
}

#[test]
fn orchestrator_menu_layout_holds_in_a_long_conversation_at_its_end() {
  let mut screen = long_conversation(3);
  attach(&mut screen);
  holds_still(&mut screen, ("Compare", "Compare @", "Compare @zz"), MENTIONS);
  holds_still(&mut screen, ("", "/", "/hist"), SLASH);
}
