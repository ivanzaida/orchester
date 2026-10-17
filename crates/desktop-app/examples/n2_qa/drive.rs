//! The example's own presses before the window shows, where a state follows a person's press in the app: *Link to
//! this project…* with the project's address typed (`upload`, `download`, `review`, `link-foreign`), then *Link and
//! upload* after a task was added (`upload-changed`); an edit saved over a newer version (`conflict`); and *Make local*
//! confirmed as the server goes away (`made-local-stale`). These are the example's input, not an MCP client's, except
//! the one press made as an MCP client's would be (`upload-person-only`); no app code path was added for them.

use std::{
  error::Error,
  time::{Duration, Instant},
};

use lurq::app::{
  App, Tree,
  events::{MouseButton, with_synthetic_input},
};

use super::{
  State,
  seed::{self, Seeded},
};

/// Brings the window to `state` as a person's presses would.
pub fn before_showing(app: &mut App, tree: &mut Tree, (state, seeded): (State, &Seeded)) -> Result<(), Box<dyn Error>> {
  match state {
    State::Upload | State::Download | State::Review | State::LinkForeign => {
      open_link_page(app, tree, seeded)?;
      until(
        app,
        tree,
        match state {
          State::Review => "sharing-join",
          State::LinkForeign => "sharing-link-error",
          _ => "sharing-upload",
        },
      )
    }
    State::UploadChanged => {
      open_link_page(app, tree, seeded)?;
      until(app, tree, "sharing-upload")?;
      seed::add_local_task(&seeded.repo, "Retry failed deliveries once an hour")?;
      press(app, tree, "sharing-link-confirm")?;
      until(app, tree, "sharing-upload-changed")
    }
    State::UploadPersonOnly => {
      open_link_page(app, tree, seeded)?;
      until(app, tree, "sharing-upload-discovery-switch")?;
      with_synthetic_input(|| press(app, tree, "sharing-upload-discovery-switch"))?;
      until(app, tree, "sharing-upload-discovery-switch-person-only")
    }
    State::NoServer | State::DiscoveryForeign => until(app, tree, "sharing-discovery-unavailable"),
    State::MadeLocalStale => {
      until(app, tree, "sharing-make-local-confirm")?;
      seeded.port.down();
      press(app, tree, "sharing-make-local-confirm")?;
      until(app, tree, "sharing-made-local")
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

/// *Link to a server project* with the project's address typed, and *Link to Kontur…* pressed.
fn open_link_page(app: &mut App, tree: &mut Tree, seeded: &Seeded) -> Result<(), Box<dyn Error>> {
  until(app, tree, "sharing-address")?;
  set(app, tree, "sharing-address", &seeded.address)?;
  press(app, tree, "sharing-link-button")
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
