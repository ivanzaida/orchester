//! What a person does on the start page's *Join a team* with a code before the window shows, with the pointer of the
//! example itself: *Join with the code* takes no MCP client's press (`join-team-code-submit`, `PersonOnly`), and Enter
//! and *Try again* join the same way, so each result of a press is a scenario of its own.

#[path = "../../sign_in_qa/window.rs"]
mod window;

use std::error::Error;

use lurq::app::{App, Tree};
use orchester_api::test_support::team_server::{CODE, LATER_CODE, TeamServer};
use orchester_app::{
  mcp::WindowLink,
  routes::{JoinWay, Route},
};
use window::Window;

use super::State;

const ADDRESS: &str = "join-team-code-address";
const CODE_FIELD: &str = "join-team-code-code";
const SUBMIT: &str = "join-team-code-submit";
const WRONG: &str = "join-team-code-wrong";
/// A code the stand-in never made.
const WRONG_CODE: &str = "WRONG123";

/// Takes the window to `state` on `server`.
pub fn to(
  app: &mut App,
  tree: &mut Tree,
  (state, link, server): (State, &WindowLink, &TeamServer),
) -> Result<(), Box<dyn Error>> {
  let mut window = Window { app, tree };
  link.navigation()?.go(Route::JoinTeam(JoinWay::Code));
  window.until(ADDRESS)?;
  window.set(ADDRESS, &server.url())?;
  match state {
    State::Join => Ok(()),
    State::WrongCode => join_with(&mut window, WRONG_CODE, WRONG),
    State::ExpiredCode => join_with(&mut window, LATER_CODE, WRONG),
    State::Unreachable => join_with(&mut window, CODE, "join-team-code-unreachable"),
    State::Joined => join_with(&mut window, CODE, "join-team-code-joined"),
    State::Offer | State::Downloading | State::DownloadFailed | State::DownloadProgress | State::DownloadStopped => {
      Err(format!("{state:?} opens the app's own window, with nothing to press").into())
    }
  }
}

/// Types `code`, presses *Join with the code*, and waits for `shows`.
fn join_with(window: &mut Window<'_>, code: &str, shows: &str) -> Result<(), Box<dyn Error>> {
  window.set(CODE_FIELD, code)?;
  window.press(SUBMIT)?;
  window.until(shows)
}
