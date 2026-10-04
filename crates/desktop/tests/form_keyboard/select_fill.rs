//! What a select's trigger paints, read from the headless paint: a select drawn like a field fills
//! with the field's `SurfaceInput` (`ZGwlv`), and the composer's recipient (`opOGv`) paints no fill
//! of its own, so the composer box's `$raised` shows through it until the pointer is on it. None of
//! them is the white lurq once filled a trigger without a background with.

use lurq::{app::theme::PaletteColor, node::color::Color};

use crate::{
  harness::Desktop,
  task_run::{fill_new_task, init_repository, programs, seed},
};

const WHITE: Color = Color::new(255, 255, 255, 255);

/// `id`'s trigger fill is `role`, never white.
fn fills_with(desktop: &mut Desktop, id: &str, role: PaletteColor) {
  let fill = desktop.fill_at(id);
  assert_ne!(fill, Some(WHITE), "{id}");
  assert_eq!(fill, Some(desktop.color(role)), "{id}");
}

#[test]
fn select_triggers_paint_their_theme_fill() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  fill_new_task(&mut desktop, "Say hello.", "1400000");
  fills_with(&mut desktop, "pipeline-selector", PaletteColor::SurfaceInput);

  desktop.click("save-draft");
  desktop.wait_for("task-row-KON-1");
  fills_with(&mut desktop, "tasks-sort", PaletteColor::SurfaceInput);
  fills_with(&mut desktop, "tasks-pipeline-filter", PaletteColor::SurfaceInput);

  desktop.click("task-row-KON-1");
  desktop.wait_for("task-tab-conversation");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-recipient");
  fills_with(&mut desktop, "conversation-recipient", PaletteColor::SurfaceRaised);
  desktop.hover("conversation-recipient");
  fills_with(&mut desktop, "conversation-recipient", PaletteColor::AccentMuted);
}
