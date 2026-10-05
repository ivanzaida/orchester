//! Probe: where a press on the last tab fails to drag it.

use lurq::app::events::MouseButton;

use crate::{
  harness::Desktop,
  tabs::{command, tab_ids},
};

fn three_tabs() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  command(&mut desktop, "t", "KeyT", false);
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  desktop
}

/// Presses `id` at (`x`, `y`) and moves 60px left; whether the tab lifted.
fn lifts(desktop: &mut Desktop, id: &str, x: f32, y: f32) -> bool {
  let before = desktop.bounds(id).x;
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  for step in 1..=6 {
    desktop.tree.mouse_move(x - 10.0 * step as f32, y);
    desktop.pass();
  }
  let moved = (desktop.bounds(id).x - before).abs() > 1.0;
  desktop.key("Escape", "Escape");
  desktop.tree.mouse_up(x - 60.0, y, MouseButton::Left);
  desktop.pass();
  std::thread::sleep(std::time::Duration::from_millis(600));
  moved
}

fn probe(desktop: &mut Desktop, id: &str, label: &str) {
  let tab = desktop.bounds(id);
  eprintln!("{label}: {id} at {tab:?}, order {:?}", tab_ids(desktop));
  let mut failed = Vec::new();
  for y in [tab.y + 1.0, tab.y + tab.height / 2.0, tab.y + tab.height - 1.0] {
    let mut x = tab.x + 1.0;
    while x < tab.x + tab.width {
      if !lifts(desktop, id, x, y) {
        failed.push((x - tab.x, y - tab.y));
      }
      x += 7.0;
    }
  }
  eprintln!("{label}: failed at {failed:?}");
}

#[test]
fn tabs_drag_probe() {
  let mut desktop = three_tabs();
  probe(&mut desktop, "window-tab-3", "last active");
  probe(&mut desktop, "window-tab-2", "middle");
  command(&mut desktop, "1", "Digit1", false);
  desktop.pass();
  probe(&mut desktop, "window-tab-3", "last inactive");
}
