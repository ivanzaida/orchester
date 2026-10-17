//! The window's tabs per Orchester Cloud account (the person's decision, 2026-10-16: projects per account). The tabs
//! stay as they were behind the sign-in gate, so the same account signing in again finds them as it left them. Another
//! account signing in gets its own tabs instead: those it left in this window earlier, or one tab at All projects; the
//! last account's tabs wait for it, so none of them shows a project to an account it is not its. The tabs a window
//! opened with while no account was signed in are the first account's to sign in, as the projects added meanwhile are.

use std::collections::HashMap;

use orchester_core::id::CloudAccountId;
use parking_lot::Mutex;

use super::{Navigation, tabs::TabSet};
use crate::project::Projects;

/// Whose tabs the window shows, and the tabs other accounts left in it.
pub struct AccountTabs(Mutex<Shown>);

struct Shown {
  account: Option<CloudAccountId>,
  kept: HashMap<CloudAccountId, TabSet>,
}

impl AccountTabs {
  /// The tabs of a window that opened while `account` was signed in (`None`: none was).
  pub fn new(account: Option<CloudAccountId>) -> Self {
    Self(Mutex::new(Shown {
      account,
      kept: HashMap::new(),
    }))
  }

  /// `account` is signed in: the window shows its tabs. When they replace another account's, what the window knew of
  /// that account's projects goes from `projects` too.
  pub fn follow(&self, account: CloudAccountId, navigation: &Navigation, projects: &Projects) {
    let mut shown = self.0.lock();
    let Some(previous) = shown.account.replace(account) else {
      return;
    };
    if previous == account {
      return;
    }
    let tabs = shown.kept.remove(&account).unwrap_or_default();
    let left = navigation.swap_tabs(tabs);
    shown.kept.insert(previous, left);
    projects.forget_all();
  }
}
