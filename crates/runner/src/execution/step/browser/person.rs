//! An agent's call that acts on a tab the person uses in the *Browser* window waits first (`docs/design/browser.md`,
//! "What the agent gets", decision 16): every call that reports what it did to the window ([`Acted`]) and
//! `browser_close`, but for a `browser_open` of a new tab, which touches no tab the person could be using. What the
//! wait came to is the first line of the call's answer, or the whole answer when the call did not act.

use std::time::Duration;

use orchester_core::orchester_tools::browser::BrowserCall;

use super::{StepBrowser, acted::Acted};
use crate::browser::PersonWait;

/// What a call that a stop of its run ended answers.
const STOPPED: &str = "This call did not act: the person was using this tab in the Browser window, and the run is \
                       stopping.";

/// Whether `call` waits while the person uses its tab; the reading tools never do.
fn acts_on_a_tab(call: &BrowserCall) -> bool {
  match call {
    BrowserCall::Open { new_tab: true, .. } => false,
    BrowserCall::Close { .. } => true,
    _ => Acted::of(call).is_some(),
  }
}

impl StepBrowser {
  /// Waits while the person uses the tab `call` acts on, and what that came to. A tab that closed or a browser that
  /// restarted meanwhile leaves the call to look again at the tab it now acts on. A tab that cannot be reached holds
  /// nothing: the call's own answer says why.
  pub(in crate::execution::step) async fn wait_for_person(&self, call: &BrowserCall) -> PersonWait {
    if !acts_on_a_tab(call) {
      return PersonWait::NotHeld;
    }
    let mut waited = Duration::ZERO;
    loop {
      let wait = match self.tab(call.tab()).await {
        Ok(tab) => self.browser.wait_for_person(self.run, tab.id()).await,
        Err(_) => PersonWait::NotHeld,
      };
      match wait {
        PersonWait::Gone(after) => waited += after,
        PersonWait::NotHeld if waited.is_zero() => return PersonWait::NotHeld,
        PersonWait::NotHeld => return PersonWait::Over(waited),
        PersonWait::Over(after) => return PersonWait::Over(waited + after),
        PersonWait::Capped(after) => return PersonWait::Capped(waited + after),
        PersonWait::Stopped => return PersonWait::Stopped,
      }
    }
  }
}

/// The line a call's answer starts with after `wait`, if any; the whole answer when the call does not act.
pub(super) fn first_line(wait: PersonWait) -> Result<Option<String>, String> {
  match wait {
    PersonWait::NotHeld | PersonWait::Gone(_) => Ok(None),
    PersonWait::Over(after) => Ok(Some(format!(
      "The person was using this tab in the Browser window, so this call waited {} before it ran.",
      seconds(after)
    ))),
    PersonWait::Capped(after) => Err(format!(
      "The person is still using this tab in the Browser window: this call waited {} and did not act. Try again \
       later, or work in another tab.",
      seconds(after)
    )),
    PersonWait::Stopped => Err(STOPPED.to_owned()),
  }
}

/// `waited` in whole seconds, rounded: a wait ends a moment after its time.
fn seconds(waited: Duration) -> String {
  format!("{} s", (waited.as_millis() + 500) / 1000)
}
