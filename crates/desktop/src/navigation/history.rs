//! Going forward in a tab's history. A finished step (a submitted form) is only ever the last
//! entry, and going forward onto it returns at once, as if history ended before it. The window's
//! input for back and forward is in `tabs::input`.

use lurq::router::RouterHandle;

use super::entry;

/// Goes forward a step; `false` when there is nowhere to go.
pub fn forward(router: &RouterHandle) -> bool {
  if !router.forward() {
    return false;
  }
  if entry::is_finished(&router.navigator()) {
    router.back();
    return false;
  }
  true
}
