//! What a person does in the window before it shows, with the pointer and the keyboard of the example itself (never an
//! MCP client's, which the gate refuses): typing and pressing *Sign in*, opening *Advanced*, resting the pointer on a
//! status item, asking to sign out, pressing Tab.

use std::{
  error::Error,
  thread,
  time::{Duration, Instant},
};

use lurq::app::{App, Tree, events::MouseButton};
use orchester_app::{
  mcp::WindowLink,
  routes::{OrchesterView, Route},
  ui::tooltip::TOOLTIP_DELAY,
};

use super::{
  State,
  signed_in::account_cloud::{EMAIL, PASSWORD, PAUSED, UNCONFIRMED},
};

/// Takes the window to `state`, then presses Tab `tabs` times.
pub fn to(
  app: &mut App,
  tree: &mut Tree,
  (state, link): (State, &WindowLink),
  tabs: usize,
) -> Result<(), Box<dyn Error>> {
  let mut window = Window { app, tree };
  reach(&mut window, (state, link))?;
  for _ in 0..tabs {
    window.tab();
  }
  Ok(())
}

fn reach(window: &mut Window<'_>, (state, link): (State, &WindowLink)) -> Result<(), Box<dyn Error>> {
  match state {
    State::FirstRun | State::Unreachable | State::Wrong | State::Unconfirmed | State::Paused | State::SigningIn => {
      window.until("gate-email")?;
      let (email, password, shows) = match state {
        State::FirstRun => return Ok(()),
        State::Wrong => (EMAIL, "not the password", "gate-wrong"),
        State::Unreachable => (EMAIL, PASSWORD, "gate-unreachable"),
        State::Unconfirmed => (UNCONFIRMED, "a password for it", "gate-unconfirmed"),
        State::Paused => (PAUSED, "a password for it", "gate-paused"),
        _ => (EMAIL, PASSWORD, "gate-checking"),
      };
      window.set("gate-email", email)?;
      window.set("gate-password", password)?;
      window.press("gate-sign-in")?;
      window.until(shows)
    }
    State::McpCard => window.rest_on("status-mcp", "status-mcp-card"),
    State::Advanced => {
      window.until("orchester-advanced")?;
      window.press("orchester-advanced")?;
      window.until("orchester-advanced-address")
    }
    State::Revoked | State::PasswordReset | State::AccountDeleted | State::Other | State::SignedOut => {
      window.until("gate-notice")
    }
    State::SignOut => {
      link.navigation()?.go(Route::Orchester(OrchesterView::default()));
      window.until("orchester-cloud-sign-out")?;
      window.press("orchester-cloud-sign-out")?;
      window.until("orchester-cloud-sign-out-question")
    }
    State::Offline => window.until("status-cloud"),
    State::OfflineCard => window.rest_on("status-cloud", "status-cloud-card"),
  }
}

struct Window<'a> {
  app: &'a mut App,
  tree: &'a mut Tree,
}

impl Window<'_> {
  /// Lays the window out, delivering what its futures finished, until `id` shows; an error after 20 seconds.
  fn until(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
      self.tree.tick_futures();
      self.tree.pass_headless(self.app);
      if self.tree.get_element_by_id(id).is_some() {
        return Ok(());
      }
      if Instant::now() > deadline {
        return Err(format!("{id} did not show").into());
      }
      thread::sleep(Duration::from_millis(10));
    }
  }

  fn center(&mut self, id: &str) -> Result<(f32, f32), Box<dyn Error>> {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .ok_or_else(|| format!("{id} is laid out"))?;
    Ok((bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0))
  }

  /// Presses the middle of `id` with the pointer.
  fn press(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
    let (x, y) = self.center(id)?;
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    // The pointer leaves for the panel's empty side, so what it pressed shows as it rests.
    self.tree.mouse_move(1.0, y);
    self.tree.pass_headless(self.app);
    Ok(())
  }

  /// Types `value` into the text input `id`, in place of what it held.
  fn set(&mut self, id: &str, value: &str) -> Result<(), Box<dyn Error>> {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .ok_or_else(|| format!("{id} is a text input"))?
      .set_value(value);
    self.tree.pass_headless(self.app);
    Ok(())
  }

  /// Presses Tab on the keyboard.
  fn tab(&mut self) {
    self.tree.key_down("Tab".into(), "Tab".into(), false, false, false);
    self.tree.key_up("Tab".into(), "Tab".into(), false, false, false);
    self.tree.pass_headless(self.app);
  }

  /// Rests the pointer on `id` until `card` shows.
  fn rest_on(&mut self, id: &str, card: &str) -> Result<(), Box<dyn Error>> {
    self.until(id)?;
    let (x, y) = self.center(id)?;
    self.tree.mouse_move(x, y);
    self.tree.pass_headless(self.app);
    self
      .tree
      .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
    self.until(card)
  }
}
