//! A skeleton pulses gently once its bones show: lurq plays its pulse on the animation clock, so the window asks for
//! frames while it is there. Its own process: whether the operating system asks for reduced motion is kept once per
//! process (`skeleton_still` holds it still).

#[path = "skeleton_host/mod.rs"]
mod skeleton_host;

use skeleton_host::Host;

#[test]
fn a_skeleton_pulses_once_its_bones_show() {
  let mut host = Host::show();
  assert!(!host.has_bones(), "no bones before the delay");
  assert!(!host.tree.has_active_timeline(), "nothing pulses before the bones show");
  host.after_delay();
  assert!(host.has_bones(), "the bones show after the delay");
  assert!(host.tree.has_active_timeline(), "the bones pulse");
}
