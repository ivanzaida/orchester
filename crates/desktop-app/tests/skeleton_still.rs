//! A skeleton stays still with reduced motion: its bones show after the delay as ever, and nothing animates. Its own
//! process: whether the operating system asks for reduced motion is kept once per process (`skeleton_pulse` lets it
//! pulse).

#[path = "skeleton_host/mod.rs"]
mod skeleton_host;

use orchester_app::ui::skeleton;
use skeleton_host::Host;

#[test]
fn a_skeleton_stays_still_with_reduced_motion() {
  skeleton::hold_still(true);
  let mut host = Host::show();
  host.after_delay();
  assert!(host.has_bones(), "the bones show after the delay");
  assert!(!host.tree.has_active_timeline(), "reduced motion: the bones stand still");
}
