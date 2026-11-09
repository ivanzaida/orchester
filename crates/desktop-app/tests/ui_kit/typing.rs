//! The typing dots (`Message · waiting` `sXET0`, `Dot` `h6EkpB`, ORC-963) move while shown: each fades from the first
//! dot's opacity to the last's and back once a `REVIEW_TYPING_CYCLE`, a third of a cycle behind the one before it,
//! and a window without dots stays idle. Their cycle is sampled against lurq's animation clock in real time.

use std::{
  thread,
  time::{Duration, Instant},
};

use lurq::{components::Text, layout::quad::QuadContent};
use orchester_app::{
  theme::{REVIEW_TYPING_CYCLE, REVIEW_TYPING_OPACITY},
  ui::typing::dots,
};

use crate::harness::Kit;

/// How often the dots are sampled.
const SAMPLE: Duration = Duration::from_millis(11);
/// What the design draws a dot's side as (`h6EkpB`).
const DOT_SIDE: f32 = 6.0;

fn shown() -> Kit {
  Kit::show(|_| dots("typing"))
}

/// The opacity each of the three dots is drawn with, left to right.
fn opacities(kit: &Kit) -> Vec<f32> {
  let layout = kit.tree.last_layout().expect("the window is laid out");
  let mut dots: Vec<(f32, f32)> = kit
    .tree
    .resolve_quads(layout)
    .into_iter()
    .filter(|quad| {
      matches!(quad.content, QuadContent::Rect { .. }) && quad.width == DOT_SIDE && quad.height == DOT_SIDE
    })
    .map(|quad| (quad.x, quad.opacity))
    .collect();
  dots.sort_by(|a, b| a.0.total_cmp(&b.0));
  dots.into_iter().map(|(_, opacity)| opacity).collect()
}

#[test]
fn typing_dots_fade_in_a_wave_over_a_cycle_each_a_third_behind_the_one_before() {
  let mut kit = shown();
  assert_eq!(opacities(&kit).len(), 3, "three dots are drawn");
  let [bright, _, dim] = REVIEW_TYPING_OPACITY;
  let mut seen: [Vec<f32>; 3] = Default::default();
  let mut apart = false;
  let end = Instant::now() + REVIEW_TYPING_CYCLE + REVIEW_TYPING_CYCLE / 4;
  while Instant::now() < end {
    kit.pass();
    let now = opacities(&kit);
    assert_eq!(now.len(), 3);
    for (dot, opacity) in now.iter().enumerate() {
      assert!(
        (dim - 0.001..=bright + 0.001).contains(opacity),
        "a dot stays between the last and the first's opacity: {opacity}"
      );
      seen[dot].push(*opacity);
    }
    apart |= (now[0] - now[1]).abs() > 0.1 && (now[1] - now[2]).abs() > 0.1;
    thread::sleep(SAMPLE);
  }
  for (dot, opacities) in seen.iter().enumerate() {
    let low = opacities.iter().copied().fold(f32::MAX, f32::min);
    let high = opacities.iter().copied().fold(f32::MIN, f32::max);
    assert!(
      high - low > 0.5,
      "dot {dot} fades through most of the range in a cycle: {low}..{high}"
    );
  }
  assert!(
    apart,
    "the dots are not in step: each is a third of a cycle behind the one before"
  );
}

#[test]
fn typing_dots_keep_the_window_animating_and_none_keeps_it_idle() {
  let kit = shown();
  assert!(kit.tree.has_active_timeline(), "dots ask for frames");
  let next = kit.tree.next_timeline_change().expect("the dots change pose");
  assert!(
    next.saturating_duration_since(Instant::now()) <= REVIEW_TYPING_CYCLE,
    "the next frame is due within a cycle"
  );
  let idle = Kit::show(|_| Text::new("nothing moves").into());
  assert!(!idle.tree.has_active_timeline(), "a window without dots is idle");
}
