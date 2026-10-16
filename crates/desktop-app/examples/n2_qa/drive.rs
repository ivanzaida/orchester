//! The example's own presses before the window shows, where a state follows a person's press in the app: *Link to
//! this project…* with the project's address typed (`upload`, `download`, `review`), and an edit saved over a newer
//! version (`conflict`). These are the example's input, not an MCP client's; no app code path was added for them.

use std::{
  error::Error,
  time::{Duration, Instant},
};

use lurq::app::{App, Tree, events::MouseButton};

use super::{State, seed::Seeded};

/// Brings the window to `state` as a person's presses would.
pub fn before_showing(app: &mut App, tree: &mut Tree, (state, seeded): (State, &Seeded)) -> Result<(), Box<dyn Error>> {
  match state {
    State::Upload | State::Download | State::Review => {
      until(app, tree, "sharing-address")?;
      set(app, tree, "sharing-address", &seeded.address)?;
      press(app, tree, "sharing-link-button")?;
      until(
        app,
        tree,
        if state == State::Review {
          "sharing-taken"
        } else {
          "sharing-upload"
        },
      )
    }
    State::Conflict => {
      until(app, tree, "task-form-title-input")?;
      set(
        app,
        tree,
        "task-form-title-input",
        "Add retry with backoff to the webhook sender",
      )?;
      seeded
        .server
        .retitle(&seeded.kontur, "KON-1", "Add retry to webhook sender, three attempts");
      press(app, tree, "task-form-save")?;
      until(app, tree, "task-form-conflict-fields")
    }
    _ => Ok(()),
  }
}

/// Lays the window out, delivering what its futures finished, until `id` shows; an error after 20 seconds.
fn until(app: &mut App, tree: &mut Tree, id: &str) -> Result<(), Box<dyn Error>> {
  let deadline = Instant::now() + Duration::from_secs(20);
  loop {
    tree.tick_futures();
    tree.pass_headless(app);
    if tree.get_element_by_id(id).is_some() {
      return Ok(());
    }
    if Instant::now() > deadline {
      return Err(format!("{id} did not show").into());
    }
    std::thread::sleep(Duration::from_millis(10));
  }
}

fn set(app: &mut App, tree: &mut Tree, id: &str, value: &str) -> Result<(), Box<dyn Error>> {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .ok_or_else(|| format!("{id} is a text input"))?
    .set_value(value.to_owned());
  tree.pass_headless(app);
  Ok(())
}

/// Presses the middle of `id` with the pointer.
fn press(app: &mut App, tree: &mut Tree, id: &str) -> Result<(), Box<dyn Error>> {
  let bounds = tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .ok_or_else(|| format!("{id} is laid out"))?;
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  tree.pass_headless(app);
  Ok(())
}
