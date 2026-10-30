//! The window's tabs per Orchester Cloud sign-in (the person's decisions, 2026-10-16 and 2026-10-30). Each account
//! keeps the tabs it left, and so does being signed out, so signing in again finds them as they were, and none of them
//! shows a project to someone it is not shown to. Everyone sees this computer's projects: the first sign-in with no
//! tabs kept for it keeps the tabs that show, which are all still visible. A sign-out with no tabs of its own opens
//! Settings › Orchester Cloud, which says why a sign-in ended and offers it again.

use std::collections::HashMap;

use orchester_core::id::CloudAccountId;
use parking_lot::Mutex;

use super::{Navigation, tabs::TabSet};
use crate::{
  project::Projects,
  routes::{OrchesterView, Route},
};

/// Whose tabs the window shows, and the tabs the others left in it.
pub struct AccountTabs(Mutex<Shown>);

struct Shown {
  account: Option<CloudAccountId>,
  kept: HashMap<Option<CloudAccountId>, TabSet>,
}

impl AccountTabs {
  /// The tabs of a window that opened while `account` was signed in (`None`: none was).
  pub fn new(account: Option<CloudAccountId>) -> Self {
    Self(Mutex::new(Shown {
      account,
      kept: HashMap::new(),
    }))
  }

  /// `account` is signed in (`None`: nobody is): the window shows its tabs. When they replace others, what the window
  /// knew of the projects those showed goes from `projects` too.
  pub fn follow(&self, account: Option<CloudAccountId>, navigation: &Navigation, projects: &Projects) {
    let mut shown = self.0.lock();
    let previous = std::mem::replace(&mut shown.account, account);
    if previous == account {
      return;
    }
    let kept = shown.kept.remove(&account);
    // Signed out to signed in with nothing kept: every tab that shows is of this computer's, and stays.
    if previous.is_none() && kept.is_none() {
      return;
    }
    let tabs = kept.unwrap_or_else(|| match account {
      Some(_) => TabSet::default(),
      // Signed out with no tabs of its own: Settings › Orchester Cloud says why, and signs in again.
      None => TabSet::at(Route::Orchester(OrchesterView::default())),
    });
    let left = navigation.swap_tabs(tabs);
    shown.kept.insert(previous, left);
    projects.forget_all();
  }
}
