//! Dragging a tab along the strip (state G `Ymhc7`): a press anywhere on a tab, its close button
//! included, moved along the strip lifts it, every tab alike: the last one too, active or not,
//! after a drop, and while the strip scrolls. Released, it lands in any slot. The window's drag
//! area never takes the press: its handler stops the press's default, which would keep the tab
//! from lifting.

use lurq::app::events::MouseButton;

use crate::{
  harness::Desktop,
  tabs::{active_tab, command, has, tab_ids},
};

/// Kontur and `more` tabs on All projects, the last one active.
fn tabs(more: usize) -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  for _ in 0..more {
    command(&mut desktop, "t", "KeyT", false);
  }
  desktop.wait_for("all-projects");
  desktop
}

fn tab(id: u64) -> String {
  format!("window-tab-{id}")
}

fn last(desktop: &mut Desktop) -> u64 {
  *tab_ids(desktop).last().expect("a tab")
}

/// Whether a press on tab `id` at (`x`, `y`), moved `dx` along the strip, lifts it. The release
/// away from the title bar puts it back.
fn lifts_at(desktop: &mut Desktop, id: u64, (x, y): (f32, f32), dx: f32) -> bool {
  let resting = desktop.bounds(&tab(id)).x;
  desktop.tree.mouse_move(x, y);
  desktop.pass();
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  for step in 1..=6 {
    desktop.tree.mouse_move(x + dx * step as f32 / 6.0, y);
    desktop.pass();
  }
  let lifted = (desktop.bounds(&tab(id)).x - resting).abs() > 1.0;
  desktop.tree.mouse_up(x + dx, 300.0, MouseButton::Left);
  desktop.pass();
  lifted
}

/// The presses along tab `id`, every 8px and over its close button's row, that do not lift it.
fn presses_that_do_not_lift(desktop: &mut Desktop, id: u64, dx: f32) -> Vec<(f32, f32)> {
  let bounds = desktop.bounds(&tab(id));
  let mut missed = Vec::new();
  for dy in [2.0, bounds.height / 2.0, bounds.height - 2.0] {
    let mut x = 2.0;
    while x < bounds.width - 1.0 {
      if !lifts_at(desktop, id, (bounds.x + x, bounds.y + dy), dx) {
        missed.push((x, dy));
      }
      x += 8.0;
    }
  }
  missed
}

/// Drags tab `id` by its middle onto the middle of the tab now in `slot`, and drops it there.
fn drag_to(desktop: &mut Desktop, id: u64, slot: usize) {
  let target = tab(tab_ids(desktop)[slot]);
  let (from, to) = (desktop.bounds(&tab(id)), desktop.bounds(&target));
  let (x, y) = (from.x + from.width / 2.0, from.y + from.height / 2.0);
  let dx = to.x + to.width / 2.0 - x;
  desktop.tree.mouse_move(x, y);
  desktop.pass();
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  for step in 1..=10 {
    desktop.tree.mouse_move(x + dx * step as f32 / 10.0, y);
    desktop.pass();
  }
  desktop.tree.mouse_up(x + dx, y, MouseButton::Left);
  desktop.pass();
}

#[test]
fn tabs_drag_starts_anywhere_on_a_tab() {
  let mut desktop = tabs(2);
  assert_eq!(tab_ids(&mut desktop), [1, 2, 3]);
  assert!(has(&mut desktop, "window-tab-close-3"));
  assert_eq!(
    presses_that_do_not_lift(&mut desktop, 3, -60.0),
    [],
    "the active last tab, its close button included"
  );
  assert_eq!(presses_that_do_not_lift(&mut desktop, 2, -60.0), [], "a middle tab");
  assert_eq!(presses_that_do_not_lift(&mut desktop, 1, 60.0), [], "the first tab");

  command(&mut desktop, "1", "Digit1", false);
  assert_eq!(
    presses_that_do_not_lift(&mut desktop, 3, -60.0),
    [],
    "the inactive last tab"
  );

  // A dropped tab loses its hover look with the pointer still on it, so its close button is not
  // shown there: a press where it is still drags.
  drag_to(&mut desktop, 3, 1);
  drag_to(&mut desktop, 3, 2);
  assert_eq!(tab_ids(&mut desktop), [1, 2, 3]);
  let close = desktop.bounds("window-tab-close-3");
  let at = (close.x + close.width / 2.0, close.y + close.height / 2.0);
  assert!(
    lifts_at(&mut desktop, 3, at, -60.0),
    "a press where the dropped tab's close button is"
  );
  assert_eq!(
    tab_ids(&mut desktop),
    [1, 2, 3],
    "released away from the title bar, it goes back"
  );
}

#[test]
fn tabs_the_last_tab_drags_to_every_slot() {
  let mut desktop = tabs(3);
  for active in [true, false] {
    for slot in 0..3 {
      let id = last(&mut desktop);
      drag_to(&mut desktop, id, slot);
      let order = tab_ids(&mut desktop);
      assert_eq!(order[slot], id, "the last tab lands in slot {slot}, active: {active}");
      drag_to(&mut desktop, id, 3);
      assert_eq!(
        *tab_ids(&mut desktop).last().expect("a tab"),
        id,
        "and back in the last slot"
      );
    }
    command(&mut desktop, "1", "Digit1", false);
  }
  assert_eq!(active_tab(&mut desktop), Some(1), "dragging does not activate a tab");
}

#[test]
fn tabs_the_last_tab_drags_while_the_strip_scrolls() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.tree.resize(1100, 960);
  desktop.pass();
  for _ in 0..6 {
    command(&mut desktop, "t", "KeyT", false);
  }
  desktop.wait_for("window-tab-list");
  // The tab list opened and closed again leaves nothing that takes the next press.
  desktop.press("window-tab-list");
  desktop.wait_for("window-tab-list-menu");
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "window-tab-list-menu"));

  let id = last(&mut desktop);
  assert_eq!(
    active_tab(&mut desktop),
    Some(id),
    "the last tab is active and scrolled into view"
  );
  assert_eq!(presses_that_do_not_lift(&mut desktop, id, -60.0), []);
  let slot = tab_ids(&mut desktop).len() - 3;
  drag_to(&mut desktop, id, slot);
  assert_eq!(tab_ids(&mut desktop)[slot], id, "it lands in a scrolled strip's slot");
}
