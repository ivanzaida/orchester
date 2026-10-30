//! What a person does in the window before it shows, with the pointer and the keyboard of the example itself (never an
//! MCP client's, which the gate refuses): typing and pressing *Sign in*, typing a code and pressing *Verify*, opening
//! *Advanced*, resting the pointer on a status item, asking to sign out, pressing Tab.

use std::{
  error::Error,
  time::{Duration, Instant},
};

use lurq::app::{App, Tree};
use orchester_app::{
  mcp::WindowLink,
  routes::{OrchesterView, Route},
  ui::tooltip::TOOLTIP_DELAY,
};

use super::{
  State,
  signed_in::account_cloud::{AccountCloud, EMAIL, PASSWORD, PAUSED, UNCONFIRMED},
  window::Window,
};

/// Takes the window to `state`, then presses Tab `tabs` times.
pub fn to(
  app: &mut App,
  tree: &mut Tree,
  (state, link, cloud): (State, &WindowLink, &AccountCloud),
  tabs: usize,
) -> Result<(), Box<dyn Error>> {
  let mut window = Window { app, tree };
  if state.code_step() {
    code_step(&mut window, state, cloud)?;
  } else {
    reach(&mut window, (state, link))?;
  }
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
    State::Revoked
    | State::PasswordReset
    | State::AccountDeleted
    | State::Other
    | State::SignedOut
    | State::SignInForCode => window.until("gate-notice"),
    State::SignOut => {
      link.navigation()?.go(Route::Orchester(OrchesterView::default()));
      window.until("orchester-cloud-sign-out")?;
      window.press("orchester-cloud-sign-out")?;
      window.until("orchester-cloud-sign-out-question")
    }
    State::Offline => window.until("status-cloud"),
    State::OfflineCard => window.rest_on("status-cloud", "status-cloud-card"),
    State::Code | State::Recovery | State::Verifying | State::WrongCode | State::TooManyCodes | State::TimedOut => {
      Err(format!("{state:?} is reached through the code step").into())
    }
  }
}

/// Signs in with the password of an account with two-factor authentication, then takes the code step to `state`.
fn code_step(window: &mut Window<'_>, state: State, cloud: &AccountCloud) -> Result<(), Box<dyn Error>> {
  window.until("gate-email")?;
  window.set("gate-email", EMAIL)?;
  window.set("gate-password", PASSWORD)?;
  window.press("gate-sign-in")?;
  window.until("gate-code-card")?;
  let next = cloud.next_code();
  let spaced = format!("{} {}", &next[..3], &next[3..]);
  let (code, shows) = match state {
    State::Recovery => {
      window.press("gate-code-switch")?;
      return window.until("gate-code-card");
    }
    State::Verifying => {
      cloud.slow_sign_in(std::time::Duration::from_secs(3600));
      (spaced, "gate-code-checking")
    }
    State::WrongCode => ("000000".to_owned(), "gate-code-note"),
    State::TooManyCodes => {
      cloud.limit_codes(true);
      (spaced, "gate-code-paused")
    }
    State::TimedOut => {
      cloud.end_challenges();
      (spaced, "gate-timed-out")
    }
    _ => return Ok(()),
  };
  window.set("gate-code", &code)?;
  window.press("gate-verify")?;
  window.until(shows)
}

impl Window<'_> {
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
