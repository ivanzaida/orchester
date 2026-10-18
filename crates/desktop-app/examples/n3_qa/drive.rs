//! The example's own presses before the window shows, where a state follows a person's press in the app: unticking
//! KON-1 and KON-5 in the review (`join-unticked`, `join-failed`), *Join and upload* as the server goes away
//! (`join-failed`) or as an MCP client's press (`join-person-only`), *Link to this project…* with Atlas's address
//! (`not-member`), *Share task* (`task-share`) and *Add a checkout folder…* with the state's folder (`landing-added`,
//! `landing-not-git`, `landing-linked`). These are the example's input, not an MCP client's, except the one press made
//! as an MCP client's would be; no app code path was added for them.

use std::{
  error::Error,
  time::{Duration, Instant},
};

use lurq::app::{
  App, Tree,
  events::{MouseButton, with_synthetic_input},
};

use super::{State, seed::Seeded};

/// Brings the window to `state` as a person's presses would.
pub fn before_showing(app: &mut App, tree: &mut Tree, (state, seeded): (State, &Seeded)) -> Result<(), Box<dyn Error>> {
  match state {
    State::JoinReview | State::LandingDirect => until(app, tree, "sharing-join-row-KON-2-locked"),
    State::JoinUnticked => untick(app, tree),
    State::JoinFailed => {
      untick(app, tree)?;
      seeded.port.down();
      press(app, tree, "sharing-join-confirm")?;
      until(app, tree, "sharing-join-failed")
    }
    State::JoinPersonOnly => {
      until(app, tree, "sharing-join-confirm")?;
      with_synthetic_input(|| press(app, tree, "sharing-join-confirm"))?;
      until(app, tree, "sharing-join-confirm-person-only")
    }
    State::NotMember => {
      until(app, tree, "sharing-address")?;
      set(app, tree, "sharing-address", &seeded.atlas)?;
      press(app, tree, "sharing-link-button")?;
      until(app, tree, "sharing-no-project")
    }
    State::TaskShare => {
      until(app, tree, "task-share")?;
      press(app, tree, "task-share")?;
      until(app, tree, "task-share-question")
    }
    State::LandingAdded | State::LandingLinked => {
      until(app, tree, "orchester-landing-add")?;
      press(app, tree, "orchester-landing-add")?;
      until(app, tree, "orchester-landing-added")
    }
    State::LandingNotGit => {
      until(app, tree, "orchester-landing-add")?;
      press(app, tree, "orchester-landing-add")?;
      until(app, tree, "orchester-landing-refused")
    }
    State::Later => {
      until(app, tree, "orchester-landing-later")?;
      press(app, tree, "orchester-landing-later")?;
      until(app, tree, "orchester-landing-waits")
    }
    _ => Ok(()),
  }
}

/// KON-1 and KON-5 unticked in the review.
fn untick(app: &mut App, tree: &mut Tree) -> Result<(), Box<dyn Error>> {
  for key in ["KON-1", "KON-5"] {
    let tick = format!("sharing-join-row-{key}-tick");
    until(app, tree, &tick)?;
    press(app, tree, &tick)?;
  }
  Ok(())
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
