//! A link in a task's conversation that says one address and opens another shows the host it opens, as the
//! Orchestrator's chat does, and a synthetic press on it opens nothing: the conversation follows its links as the
//! window does (`ui::chat::links`).

use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  layout::quad::QuadContent,
};

use super::{Window, agent_text::agent_says, key_of, open, seeded};

/// Says one address, opens another.
const LYING: &str = "The fix is in [github.com/acme/kontur](https://evil.example/login).";

/// Every text the window draws, a link's trailing runs included.
pub fn drawn(window: &mut Window) -> Vec<String> {
  window.pass();
  let tree = &window.tree;
  tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .flat_map(|quad| match quad.content {
      QuadContent::RichText { spans, .. } => spans.into_iter().map(|span| span.text).collect(),
      QuadContent::Text { text, .. } => vec![text],
      _ => Vec::new(),
    })
    .collect()
}

/// Presses the middle of the element whose text is `words`, as the pointer does.
pub fn press(window: &mut Window, words: &str) {
  let bounds = window
    .tree
    .find_element(|element| element.text_content() == Some(words))
    .unwrap_or_else(|| panic!("{words} is a link's words"))
    .bounds();
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.mouse_down(x, y, MouseButton::Left);
  window.tree.mouse_up(x, y, MouseButton::Left);
  window.pass();
}

#[test]
fn conversation_link_shows_the_host_it_opens_and_synthetic_input_opens_nothing() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  agent_says(&seeded, &key, LYING);
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the message", |window| {
    window
      .tree
      .find_element(|element| element.text_content() == Some("github.com/acme/kontur"))
      .is_some()
  });
  assert!(
    drawn(&mut window).iter().any(|text| text == "evil.example"),
    "the link shows the host it opens"
  );
  let before = window.navigation().route_now();
  with_synthetic_input(|| press(&mut window, "github.com/acme/kontur"));
  assert_eq!(
    window.navigation().route_now(),
    before,
    "a synthetic press opens nothing"
  );
}
