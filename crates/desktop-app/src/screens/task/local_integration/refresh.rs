//! The page re-reads every two seconds. Each re-read first takes the cheap fingerprint of what the view shows and
//! reads the whole view again only when it changed, when another snapshot is shown, when the project's secrets or
//! settings changed, or after the page's own work. A whole read keeps the last view's external evidence while
//! what it was read from is unchanged.
use std::sync::Arc;

use orchester_core::{blob::ContentHash, id::RunId, secret::SecretVault};
use orchester_workspace::tasks::local_integration::{self, ShownSnapshot};
use parking_lot::Mutex;

use super::{Shown, work::Problem};

#[derive(Clone, Default)]
pub(super) struct LastRead(Arc<Mutex<Option<Entry>>>);

struct Entry {
  /// `None` once the page's own work changed what the view reads: the next read is whole.
  fingerprint: Option<ContentHash>,
  selected: Option<ShownSnapshot>,
  policy: u64,
  shown: Shown,
}

impl LastRead {
  /// The next read takes the whole view again, e.g. after the page's own work changed what it reads.
  pub(super) fn forget(&self) {
    if let Some(entry) = self.0.lock().as_mut() {
      entry.fingerprint = None;
    }
  }

  /// The view of `run` in `root` with `selected` shown, reused while its fingerprint is unchanged.
  pub(super) fn read(
    &self,
    (root, run): (&std::path::Path, RunId),
    selected: Option<&ShownSnapshot>,
    policy: u64,
    vault: &dyn SecretVault,
  ) -> Result<Shown, Problem> {
    // A fingerprint that cannot be taken only costs the shortcut: the whole read below reports its own refusal.
    let fingerprint = local_integration::fingerprint(root, run).ok();
    let previous = {
      let last = self.0.lock();
      match last.as_ref() {
        Some(last)
          if fingerprint.is_some()
            && last.fingerprint == fingerprint
            && last.selected.as_ref() == selected
            && last.policy == policy =>
        {
          return Ok(last.shown.clone());
        }
        Some(last) => Some(Arc::clone(&last.shown.0)),
        None => None,
      }
    };
    let view =
      local_integration::read_after(root, run, (selected, vault), previous.as_deref()).map_err(super::work::problem)?;
    let shown = Shown(Arc::new(view), policy);
    *self.0.lock() = Some(Entry {
      fingerprint,
      selected: selected.cloned(),
      policy,
      shown: shown.clone(),
    });
    Ok(shown)
  }
}
