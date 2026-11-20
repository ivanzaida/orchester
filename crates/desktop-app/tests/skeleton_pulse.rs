//! A skeleton pulses gently once its bones show: lurq plays its bones' pulse on the animation clock in the theme's
//! steps, so the window asks for a frame only when a pose changes.

#[path = "skeleton_host/mod.rs"]
mod skeleton_host;

use skeleton_host::Host;

#[test]
fn a_skeleton_pulses_once_its_bones_show() {
  let mut host = Host::show(false);
  let motion = host.app.theme().skeleton_motion();
  assert_eq!(motion.steps_per_second(), Some(10), "16 poses in a pulse of 1.6 s");
  assert_eq!(motion.period_length(), orchester_app::theme::SKELETON_PULSE);
  assert!(
    (motion.dimmed() - orchester_app::theme::SKELETON_DIM).abs() < 0.005,
    "a pulse dims to the design's 0.55"
  );
  assert!(!host.has_bones(), "no bones before the delay");
  assert!(!host.tree.has_active_timeline(), "nothing pulses before the bones show");
  host.after_delay();
  assert!(host.has_bones(), "the bones show after the delay");
  assert!(host.tree.has_active_timeline(), "the bones pulse");
}
