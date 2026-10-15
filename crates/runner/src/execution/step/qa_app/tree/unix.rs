//! The script's tree as the process group it leads (macOS).

use std::{io, process::Child, time::Duration};

use orchester_exec::process_group::ProcessGroup;

/// How long the tree's processes may take to exit after SIGTERM before they get SIGKILL.
const GRACE: Duration = Duration::from_secs(3);

/// Dropping it ends every process of the group.
pub(in super::super) struct Tree {
  group: Option<ProcessGroup>,
}

impl Tree {
  /// The tree of `child`, which leads its process group from before it ran.
  pub(in super::super) fn of(child: &Child) -> io::Result<Self> {
    let group = ProcessGroup::led_by(child.id()).ok_or_else(|| io::Error::other("invalid process id"))?;
    Ok(Self { group: Some(group) })
  }

  /// Lets what the group still runs (the QA app) outlive it.
  pub(in super::super) fn keep(mut self) -> io::Result<()> {
    self.group = None;
    Ok(())
  }
}

impl Drop for Tree {
  fn drop(&mut self) {
    if let Some(group) = self.group.take()
      && let Err(error) = group.end(GRACE)
    {
      tracing::warn!(%error, "the QA script's process group was not ended");
    }
  }
}
