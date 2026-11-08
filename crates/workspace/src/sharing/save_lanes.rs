//! The lane that orders the saves of one resource's draft. A panel that edits a draft starts a save for every change of
//! its fields; a save of a panel that was closed may still be on the blocking pool when the next panel of the same
//! resource saves, so the lane belongs to the resource and outlives any panel.

use std::{path::Path, sync::Arc};

use orchester_core::id::ResourceId;

use super::Servers;
use crate::agents::save_lane::SaveLane;

impl Servers {
  /// The lane of resource `id` of the project at `root`, which every holder of these `Servers` that names them shares.
  /// An entry is never removed: it is a pair of atomics and a mutex per resource that was edited, so the map does not
  /// grow with saves. The folder is named by its real path, so a symlinked path and the folder itself share one lane.
  pub fn save_lane(&self, root: &Path, id: ResourceId) -> Arc<SaveLane> {
    // A folder that cannot be resolved (it is gone) has nothing to save; its own path then names its lane.
    let key = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    self.inner.saving.lock().entry((key, id)).or_default().clone()
  }
}
