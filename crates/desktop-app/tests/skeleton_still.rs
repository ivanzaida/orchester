//! A skeleton stays still with reduced motion: its bones show after the delay as ever, and nothing animates.

#[path = "skeleton_host/mod.rs"]
mod skeleton_host;

use skeleton_host::Host;

#[test]
fn a_skeleton_stays_still_with_reduced_motion() {
  let mut host = Host::show(true);
  host.after_delay();
  assert!(host.has_bones(), "the bones show after the delay");
  assert!(
    !host.tree.has_active_timeline(),
    "reduced motion: the bones stand still"
  );
}
