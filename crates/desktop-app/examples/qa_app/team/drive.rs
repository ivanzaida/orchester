//! What a person does on *Connect a server* before the window shows, with the pointer of the example itself: *Join*
//! takes no MCP client's press (`orchester-connect-join`, `PersonOnly`), so each result of a press is a scenario of its
//! own.

#[path = "../../gate_qa/window.rs"]
mod window;

use std::error::Error;

use lurq::app::{App, Tree};
use orchester_api::test_support::team_server::{CODE, LATER_CODE, TeamServer};
use orchester_app::{
  mcp::WindowLink,
  routes::{OrchesterView, Route},
};
use window::Window;

use super::State;

const ADDRESS: &str = "orchester-connect-address";
const CONTINUE: &str = "orchester-connect-continue";
const JOIN_CODE: &str = "orchester-connect-code";
const JOIN: &str = "orchester-connect-join";
const JOIN_ERROR: &str = "orchester-connect-join-error";
/// A code the stand-in never made.
const WRONG_CODE: &str = "WRONG123";

/// Takes the window to `state` on `server`.
pub fn to(
  app: &mut App,
  tree: &mut Tree,
  (state, link, server): (State, &WindowLink, &TeamServer),
) -> Result<(), Box<dyn Error>> {
  let mut window = Window { app, tree };
  link.navigation()?.go(Route::Orchester(OrchesterView::connect()));
  window.until(ADDRESS)?;
  window.set(ADDRESS, &server.url())?;
  match state {
    State::Join => Ok(()),
    State::Unreachable => {
      window.press(CONTINUE)?;
      window.until("orchester-connect-address-error")
    }
    State::WrongCode => join_with(&mut window, WRONG_CODE, JOIN_ERROR),
    State::ExpiredCode => join_with(&mut window, LATER_CODE, JOIN_ERROR),
    State::Joined => join_with(&mut window, CODE, "orchester-connect-joined"),
    State::Offer | State::Downloading | State::DownloadFailed => {
      Err(format!("{state:?} opens the app's own window, with nothing to press").into())
    }
  }
}

/// Presses *Continue*, types `code`, presses *Join*, and waits for `shows`.
fn join_with(window: &mut Window<'_>, code: &str, shows: &str) -> Result<(), Box<dyn Error>> {
  window.press(CONTINUE)?;
  window.until(JOIN_CODE)?;
  window.set(JOIN_CODE, code)?;
  window.press(JOIN)?;
  window.until(shows)
}
