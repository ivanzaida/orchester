//! The pipeline canvas's zoom and pan: the steps the buttons and shortcuts take, the wheel's
//! continuous zoom, the range it is held in, fitting a pipeline into view, zooming around a
//! point and panning within the zoomed content.

use orchester_core::id::ResourceId;
use orchester_desktop::pipelines::zoom::{SessionZooms, Size, View, Zoom};

fn size(width: f32, height: f32) -> Size {
  Size { width, height }
}

fn close(left: f32, right: f32) -> bool {
  (left - right).abs() < 0.01
}

#[test]
fn zoom_steps_through_the_levels_and_stops_at_either_end() {
  assert_eq!(Zoom::FULL.zoomed_in().percent(), 110);
  assert_eq!(Zoom::FULL.zoomed_out().percent(), 90);
  // A level between two steps goes to the next step, not a full step further.
  assert_eq!(Zoom::new(0.8).zoomed_in().percent(), 90);
  assert_eq!(Zoom::new(0.8).zoomed_out().percent(), 75);
  let mut zoom = Zoom::FULL;
  for _ in 0..20 {
    zoom = zoom.zoomed_out();
  }
  assert_eq!(zoom, Zoom::MIN);
  assert_eq!(zoom.percent(), 25);
  for _ in 0..20 {
    zoom = zoom.zoomed_in();
  }
  assert_eq!(zoom, Zoom::MAX);
  assert_eq!(zoom.percent(), 200);
}

#[test]
fn zoom_is_held_between_25_and_200_percent() {
  assert_eq!(Zoom::new(0.01), Zoom::MIN);
  assert_eq!(Zoom::new(9.0), Zoom::MAX);
  assert_eq!(Zoom::new(f32::NAN), Zoom::FULL);
  assert_eq!(Zoom::new(f32::INFINITY), Zoom::MAX);
}

#[test]
fn zoom_wheel_zooms_in_away_from_the_user_by_ten_percent_a_notch() {
  assert_eq!(Zoom::FULL.wheeled(40.0).percent(), 110);
  assert_eq!(Zoom::FULL.wheeled(-40.0).percent(), 91);
  assert_eq!(Zoom::FULL.wheeled(120.0).percent(), 133);
  assert_eq!(Zoom::MAX.wheeled(400.0), Zoom::MAX);
}

#[test]
fn zoom_fit_shows_the_whole_pipeline_but_never_enlarges_it() {
  let content = size(1000.0, 800.0);
  assert!(close(Zoom::fitting(content, size(500.0, 800.0)).factor(), 0.5));
  assert!(close(Zoom::fitting(content, size(1000.0, 400.0)).factor(), 0.5));
  assert_eq!(Zoom::fitting(content, size(3000.0, 3000.0)), Zoom::FULL);
  assert_eq!(Zoom::fitting(size(40_000.0, 800.0), size(500.0, 500.0)), Zoom::MIN);
  // Before the canvas is laid out it has no size; nothing is fitted.
  assert_eq!(Zoom::fitting(content, size(0.0, 0.0)), Zoom::FULL);
  let fitted = View::fit(content, size(500.0, 800.0));
  assert_eq!(fitted.offset(), (0.0, 0.0));
}

#[test]
fn zoom_around_a_point_keeps_the_content_under_it() {
  let (content, viewport) = (size(2000.0, 2000.0), size(800.0, 600.0));
  let anchor = (400.0, 300.0);
  let zoomed = View::new(Zoom::FULL).zoomed(Zoom::new(2.0), anchor, content, viewport);
  // The content point (400, 300) is at (800, 600) at 200 %, scrolled back under the anchor.
  assert_eq!(zoomed.offset(), (400.0, 300.0));
  let back = zoomed.zoomed(Zoom::FULL, anchor, content, viewport);
  assert_eq!(back.offset(), (0.0, 0.0));
}

#[test]
fn pan_stays_within_the_zoomed_content() {
  let (content, viewport) = (size(1000.0, 800.0), size(500.0, 400.0));
  let view = View::new(Zoom::FULL);
  // Dragging the content right and down at the top left shows nothing beyond it.
  assert_eq!(view.panned(50.0, 50.0, content, viewport).offset(), (0.0, 0.0));
  assert_eq!(view.panned(-100.0, -60.0, content, viewport).offset(), (100.0, 60.0));
  assert_eq!(
    view.panned(-5000.0, -5000.0, content, viewport).offset(),
    (500.0, 400.0)
  );
  // Zoomed out so the content fits, it stays at the top left.
  let small = View::new(Zoom::new(0.25)).panned(-300.0, -300.0, content, viewport);
  assert_eq!(small.offset(), (0.0, 0.0));
  // Zooming out pulls an offset past the new end back in.
  let far = view.panned(-500.0, -400.0, content, viewport);
  let out = far.zoomed(Zoom::new(0.75), (0.0, 0.0), content, viewport);
  assert_eq!(out.offset(), (250.0, 200.0));
}

#[test]
fn zoom_is_remembered_per_pipeline_for_the_session() {
  let zooms = SessionZooms::default();
  let (first, second) = (ResourceId::generate(), ResourceId::generate());
  assert_eq!(zooms.get(first), Zoom::FULL);
  zooms.set(first, Zoom::new(0.5));
  assert_eq!(zooms.get(first), Zoom::new(0.5));
  assert_eq!(zooms.get(second), Zoom::FULL);
  // Another handle to the session shares what was remembered.
  let shared = zooms.clone();
  shared.set(second, Zoom::MAX);
  assert_eq!(zooms.get(second), Zoom::MAX);
}
