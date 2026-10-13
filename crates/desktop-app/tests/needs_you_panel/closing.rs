//! Every way the panel closes leaves the pill able to open it again: its close, the pill, Esc, a press outside,
//! an item that opens its task, *Show all*, and going elsewhere in the window.

use lurq::app::events::MouseButton;

use super::{Window, click, expand_approval, item_of, open, seeded, summary_of};

/// Waits for the panel to close after `how`, and opens it again from the pill.
fn closes_and_opens_again(window: &mut Window, how: &str) {
  window.until(&format!("the panel to close with {how}"), |window| {
    window.texts("needs-you-panel").is_none()
  });
  open(window);
}

fn escape(window: &mut Window) {
  window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window
    .tree
    .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window.pass();
}

#[test]
fn needs_you_panel_opens_again_after_each_way_it_closes() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  click(&mut window, "needs-you-close");
  closes_and_opens_again(&mut window, "its close");
  click(&mut window, "needs-you-pill");
  closes_and_opens_again(&mut window, "the pill");
  escape(&mut window);
  closes_and_opens_again(&mut window, "Esc");
  // A press on the page, away from the panel and the pill.
  window.tree.mouse_move(300.0, 600.0);
  window.tree.mouse_down(300.0, 600.0, MouseButton::Left);
  window.tree.mouse_up(300.0, 600.0, MouseButton::Left);
  window.pass();
  closes_and_opens_again(&mut window, "a press outside");
  // Review expands an approval in the panel, which stays open.
  let (item, _) = expand_approval(&mut window, "Add retry to webhook sender");
  assert!(window.texts("needs-you-panel").is_some(), "Review keeps the panel open");
  click(&mut window, &format!("{item}-collapse"));
  let plan = item_of(&mut window, "Plan the GitHub app permissions");
  click(&mut window, &format!("{plan}-action"));
  closes_and_opens_again(&mut window, "opening a plan");
  click(&mut window, "needs-you-show-all");
  closes_and_opens_again(&mut window, "Show all");
  window
    .navigation()
    .go(summary_of(&seeded, "Plan the GitHub app permissions"));
  window.pass();
  assert!(
    window.texts("needs-you-panel").is_some(),
    "going elsewhere by other means leaves it open"
  );
  click(&mut window, "needs-you-pill");
  closes_and_opens_again(&mut window, "the pill after going elsewhere");
}
