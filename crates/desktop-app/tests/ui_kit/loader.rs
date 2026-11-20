//! The loader (`loader-circle`) turns once a `SPIN_PERIOD` in `SPIN_STEPS` even poses, every loader on lurq's one
//! animation clock, and a window asks for frames only while a loader is in its tree. Each pass is sampled
//! against that clock in real time, so a turn that ran fast (a step taken as a whole turn) or out of phase
//! fails.

use std::{
  collections::BTreeSet,
  f32::consts::TAU,
  thread,
  time::{Duration, Instant},
};

use lurq::{
  animation::clock_origin,
  app::theme::{PaletteColor, SpacingSize},
  components::Row,
  layout::quad::QuadContent,
  ui::SPIN_PERIOD,
};
use orchester_app::{
  theme::{ExtraColor, IconSize, SPIN_STEPS},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

use crate::harness::Kit;

/// How often the turn is sampled: several times a pose.
const SAMPLE: Duration = Duration::from_millis(7);

/// A window with loaders of three sizes and a loading button, as the Tasks list and the status bar show
/// several at once.
fn loaders() -> Kit {
  Kit::show(|_| {
    Row::new()
      .spacing(SpacingSize::Lg)
      .child(icon(Icon::LoaderCircle, IconSize::Sm, PaletteColor::MutedForeground))
      .child(icon(Icon::LoaderCircle, IconSize::Md, ExtraColor::TextSecondary.role()))
      .child(icon(Icon::LoaderCircle, IconSize::Lg, ExtraColor::TextSecondary.role()))
      .child(
        ActionButton::new("busy", ButtonKind::Primary, "Create & run")
          .icon(Icon::Play)
          .on_click(|| {})
          .loading(),
      )
      .into()
  })
}

/// How far each loader glyph the window draws has turned, in radians from upright.
fn turns(kit: &Kit) -> Vec<f32> {
  let loader = Icon::LoaderCircle.glyph().to_string();
  let layout = kit.tree.last_layout().expect("the window is laid out");
  kit
    .tree
    .resolve_quads(layout)
    .into_iter()
    .filter(|quad| matches!(&quad.content, QuadContent::Text { text, .. } if *text == loader))
    .map(|quad| quad.transform.b.atan2(quad.transform.a).rem_euclid(TAU))
    .collect()
}

/// The pose the clock shows at `at`, counted in steps from upright.
fn pose_at(at: Instant) -> u32 {
  let turn = SPIN_PERIOD.as_secs_f64();
  let into_turn = at.duration_since(clock_origin()).as_secs_f64() % turn / turn;
  (into_turn * f64::from(SPIN_STEPS)).floor() as u32
}

#[test]
fn loaders_turn_once_a_second_in_24_poses_15_degrees_apart_all_in_phase() {
  assert_eq!(SPIN_STEPS, 24, "24 poses a turn");
  assert_eq!(SPIN_PERIOD, Duration::from_secs(1), "one turn a second");
  let step = TAU / SPIN_STEPS as f32;
  let mut kit = loaders();
  assert_eq!(turns(&kit).len(), 4, "every loader is drawn");

  let mut seen = BTreeSet::new();
  let end = Instant::now() + SPIN_PERIOD + SPIN_PERIOD / 4;
  while Instant::now() < end {
    let before = Instant::now();
    kit.pass();
    let after = Instant::now();
    let pose = pose_at(before);
    // A pass that crossed a step may show either pose; the next sample checks the new one.
    if pose == pose_at(after) {
      for turned in turns(&kit) {
        let expected = pose as f32 * step;
        let off = (turned - expected).abs().min(TAU - (turned - expected).abs());
        assert!(
          off < 1e-3,
          "pose {pose} is {expected} rad on the clock, the loader shows {turned}"
        );
      }
      seen.insert(pose);
    }
    thread::sleep(SAMPLE);
  }
  assert_eq!(seen.len(), SPIN_STEPS as usize, "a turn shows every pose: {seen:?}");
}

#[test]
fn a_loader_keeps_the_window_animating_one_frame_a_pose_and_none_keeps_it_idle() {
  let kit = loaders();
  assert!(kit.tree.has_active_timeline(), "a loader asks for frames");
  let next = kit.tree.next_timeline_change().expect("a turning loader changes pose");
  let pose = SPIN_PERIOD / SPIN_STEPS;
  assert!(
    next.saturating_duration_since(Instant::now()) <= pose,
    "the next frame is due within a pose ({pose:?})"
  );

  let idle = Kit::show(|_| icon(Icon::Check, IconSize::Md, ExtraColor::TextSecondary.role()).into());
  assert!(!idle.tree.has_active_timeline(), "a window without a loader is idle");
  assert_eq!(idle.tree.next_timeline_change(), None, "and asks for no frame");
}

#[test]
fn a_turning_window_asks_for_its_frames_a_pose_apart() {
  let mut kit = loaders();
  let pose = (SPIN_PERIOD / SPIN_STEPS).as_secs_f64();
  let mut due = Vec::new();
  for _ in 0..4 {
    let next = kit.tree.next_timeline_change().expect("a turning loader changes pose");
    thread::sleep(next.saturating_duration_since(Instant::now()));
    kit.pass();
    due.push(next);
  }
  for pair in due.windows(2) {
    let apart = pair[1].duration_since(pair[0]).as_secs_f64();
    let poses = (apart / pose).round();
    assert!(
      poses >= 1.0 && (apart - poses * pose).abs() < 0.001,
      "frames come on the pose grid, {pose} s apart: {apart} s"
    );
  }
}

#[test]
fn loaders_stand_still_and_ask_for_no_frames_where_motion_is_reduced() {
  let mut kit = loaders();
  kit.app.theme().set_reduced_motion(Some(true));
  kit.pass();
  assert!(!kit.tree.has_active_timeline(), "still loaders ask for no frames");
  assert!(
    turns(&kit).iter().all(|turned| turned.abs() < 1e-3),
    "every loader stands upright: {:?}",
    turns(&kit)
  );
}
