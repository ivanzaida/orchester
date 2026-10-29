//! Buttons, icon buttons and unavailable actions.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::{
    events::{MouseButton, mouse_event::MouseEvent},
    theme::{RadiusSize, SpacingSize},
  },
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_app::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  icon_button::IconButton,
  pressable::{Focus, in_title_bar, pressable},
  table::{ColumnWidth, TableRow},
  tabs::{Segment, SegmentFace, segmented},
  toggle::{Toggle, ToggleKind},
  unavailable::{reason_line, with_reason},
};

use crate::harness::{Kit, assert_close, gap_after, gap_under};

const KINDS: [(&str, ButtonKind); 4] = [
  ("primary", ButtonKind::Primary),
  ("secondary", ButtonKind::Secondary),
  ("ghost", ButtonKind::Ghost),
  ("danger", ButtonKind::Danger),
];

fn counter() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync + Clone + 'static) {
  let count = Arc::new(AtomicUsize::new(0));
  let presses = count.clone();
  (count, move || {
    presses.fetch_add(1, Ordering::SeqCst);
  })
}

#[test]
fn buttons_are_28_high_with_12_at_their_sides_and_6_from_icon_to_label() {
  let mut kit = Kit::show(|_| {
    Row::new()
      .spacing(SpacingSize::Lg)
      .with_children(KINDS.map(|(id, kind)| {
        Element::from(
          ActionButton::new(id, kind, "Continue all")
            .icon(Icon::Play)
            .on_click(|| {}),
        )
      }))
      .into()
  });
  for (id, _) in KINDS {
    let (frame, glyph, label) = (
      kit.bounds(id),
      kit.bounds(&format!("{id}-icon")),
      kit.bounds(&format!("{id}-label")),
    );
    assert_close(frame.height, 28.0, &format!("{id} height"));
    assert_close(glyph.x - frame.x, 12.0, &format!("{id} start"));
    assert_close(
      frame.x + frame.width - label.x - label.width,
      12.0,
      &format!("{id} end"),
    );
    assert_close(gap_after(glyph, label), 6.0, &format!("{id} icon to label"));
    assert_close(glyph.height, 14.0, &format!("{id} icon"));
  }
  assert_eq!(kit.radius(RadiusSize::Md), 6.0, "a button's radius, `radius_md`");
}

#[test]
fn a_button_presses_once_and_not_while_its_request_is_under_way() {
  let (presses, press) = counter();
  let (loading_presses, loading_press) = counter();
  let mut kit = Kit::show(move |_| {
    Row::new()
      .child(ActionButton::new("go", ButtonKind::Primary, "Create & run").on_click(press.clone()))
      .child(
        ActionButton::new("busy", ButtonKind::Primary, "Create & run")
          .icon(Icon::Play)
          .on_click(loading_press.clone())
          .loading(),
      )
      .into()
  });
  kit.click("go");
  kit.click("busy");
  assert_eq!(presses.load(Ordering::SeqCst), 1);
  assert_eq!(
    loading_presses.load(Ordering::SeqCst),
    0,
    "a loading button cannot be pressed"
  );
  let loader = Icon::LoaderCircle.glyph().to_string();
  assert!(
    kit.texts("busy").contains(&loader),
    "the loader turns in place of its icon"
  );
  assert!(!kit.texts("busy").contains(&Icon::Play.glyph().to_string()));
}

#[test]
fn an_unavailable_action_shows_its_lock_and_says_why_under_the_actions() {
  let mut kit = Kit::show(|_| {
    let actions = Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new("multiple", ButtonKind::Secondary, "Create multiple")
          .icon(Icon::Plus)
          .unavailable("Creating several tasks at once is planned."),
      )
      .child(
        ActionButton::new("new", ButtonKind::Primary, "New task")
          .icon(Icon::Plus)
          .on_click(|| {}),
      );
    Row::new()
      .child(with_reason(
        actions,
        "why",
        "Creating several tasks at once is planned.",
      ))
      .into()
  });
  let texts = kit.texts("multiple");
  assert!(texts.contains(&Icon::Lock.glyph().to_string()), "the lock: {texts:?}");
  assert!(
    !texts.contains(&Icon::Plus.glyph().to_string()),
    "the lock replaces its icon"
  );
  assert_eq!(kit.texts("why"), ["Creating several tasks at once is planned."]);
  let (new, why) = (kit.bounds("new"), kit.bounds("why"));
  assert_close(gap_under(new, why), 6.0, "reason under the actions");
  assert_close(new.x + new.width, why.x + why.width, "reason at the actions' end");
}

#[test]
fn a_reason_line_is_a_lock_and_the_words_6_apart() {
  let mut kit = Kit::show(|_| {
    reason_line(
      "pr",
      "Review PR needs a pull request, which no pipeline step opens yet.",
    )
    .into()
  });
  assert_eq!(
    kit.texts("pr"),
    [
      Icon::Lock.glyph().to_string(),
      "Review PR needs a pull request, which no pipeline step opens yet.".to_owned()
    ]
  );
}

#[test]
fn a_reason_line_wraps_within_its_column() {
  let mut kit = Kit::show(|_| {
    Column::new()
      .width(Dimension::Px(200.0))
      .child(reason_line(
        "offline",
        "The runner is not connected; the run's controls return once it is online.",
      ))
      .into()
  });
  let (line, words) = (kit.bounds("offline"), kit.bounds("offline-text"));
  assert_close(line.width, 200.0, "the line is as wide as its column");
  assert!(
    words.x + words.width <= line.x + line.width + 0.5,
    "the words end inside the column"
  );
  assert!(
    words.height > 20.0,
    "the words wrap onto a second line: {}",
    words.height
  );
}

#[test]
fn icon_buttons_are_28_or_24_square_and_say_what_they_do_on_hover() {
  let (presses, press) = counter();
  let mut kit = Kit::show(move |ctx| {
    Row::new()
      .child(
        IconButton::new("settings", Icon::Settings, "Settings")
          .on_click(press.clone())
          .render(ctx),
      )
      .child(
        IconButton::new("remove", Icon::X, "Remove criterion")
          .small()
          .on_click(|| {})
          .render(ctx),
      )
      .into()
  });
  let (regular, small) = (kit.bounds("settings"), kit.bounds("remove"));
  assert_eq!((regular.width, regular.height), (28.0, 28.0));
  assert_eq!((small.width, small.height), (24.0, 24.0));
  let glyph = kit.bounds("settings-icon");
  assert_close(glyph.height, 16.0, "the regular glyph");
  assert_close(glyph.y + glyph.height / 2.0, regular.y + 14.0, "the glyph is centered");
  kit.click("settings");
  assert_eq!(presses.load(Ordering::SeqCst), 1);
  kit.rest_on("remove");
  assert_eq!(kit.texts("remove-tooltip"), ["Remove criterion"]);
}

#[test]
fn a_busy_button_stays_busy_whatever_the_order_it_is_built_in() {
  let (presses, press) = counter();
  let mut kit = Kit::show(move |_| {
    Row::new()
      .child(
        ActionButton::new("loading", ButtonKind::Primary, "Create & run")
          .loading()
          .on_click(press.clone()),
      )
      .child(
        ActionButton::new("disabled", ButtonKind::Secondary, "Save")
          .disabled()
          .on_click(press.clone()),
      )
      .child(
        ActionButton::new("unavailable", ButtonKind::Secondary, "Review PR")
          .unavailable("Review PR needs a pull request.")
          .on_click(press.clone()),
      )
      .into()
  });
  for id in ["loading", "disabled", "unavailable"] {
    kit.click(id);
  }
  assert_eq!(
    presses.load(Ordering::SeqCst),
    0,
    "a hold wins over a handler given after it"
  );
  assert!(kit.texts("loading").contains(&Icon::LoaderCircle.glyph().to_string()));
  assert!(kit.texts("unavailable").contains(&Icon::Lock.glyph().to_string()));
}

#[test]
fn controls_that_cannot_be_pressed_or_flip_do_not_open_the_row_around_them() {
  let (opened, open) = counter();
  let (flips, flip) = counter();
  let mut kit = Kit::show(move |ctx| {
    let flip = flip.clone();
    let controls = Row::new()
      .spacing(SpacingSize::Sm)
      .child(ActionButton::new("busy", ButtonKind::Primary, "Run").loading())
      .child(ActionButton::new("off", ButtonKind::Secondary, "Save").disabled())
      .child(ActionButton::new("locked", ButtonKind::Secondary, "Review PR").unavailable("No pull request yet."))
      .child(IconButton::new("remove", Icon::X, "Remove").small().render(ctx))
      .child(Toggle::new("fixed", ToggleKind::Checkbox, true, "From the profile"))
      .child(Toggle::new("flip", ToggleKind::Switch, false, "Network").on_change(move |_| flip()))
      .child(segmented(
        "view",
        [Segment::new("all", SegmentFace::Text("All".into()), true, || {})],
        Focus::Pointer,
      ));
    Column::new()
      .width(Dimension::Px(1000.0))
      .child(
        TableRow::new("row")
          .cell(ColumnWidth::Fill, controls)
          .on_open(open.clone()),
      )
      .into()
  });
  for id in ["busy", "off", "locked", "remove", "fixed", "flip", "all"] {
    kit.click(id);
  }
  assert_eq!(flips.load(Ordering::SeqCst), 1, "the switch flips");
  assert_eq!(opened.load(Ordering::SeqCst), 0, "no press inside reaches the row");
  kit.click("row");
  assert_eq!(opened.load(Ordering::SeqCst), 1, "a press on the row itself opens it");
}

#[test]
fn an_unavailable_upload_keeps_its_glyph_and_cannot_press_a_handler() {
  let (presses, press) = counter();
  let mut kit = Kit::show(move |_| {
    ActionButton::new("publish", ButtonKind::Primary, "Publish")
      .icon(Icon::Upload)
      .unavailable_with_icon("There is no saved draft to publish.")
      .on_click(press.clone())
      .into()
  });
  assert!(kit.texts("publish").contains(&Icon::Upload.glyph().to_string()));
  assert!(!kit.texts("publish").contains(&Icon::Lock.glyph().to_string()));
  kit.click("publish");
  assert_eq!(presses.load(Ordering::SeqCst), 0);
}

#[test]
fn a_select_keeps_its_label_at_the_start_and_its_chevron_at_the_end_bounded_or_not() {
  let mut kit = Kit::show(|_| {
    Column::new()
      .width(Dimension::Px(300.0))
      .child(ActionButton::new("plain", ButtonKind::Select, "Agent").on_click(|| {}))
      .child(
        ActionButton::new("bounded", ButtonKind::Select, "Agent")
          .bounded_label()
          .on_click(|| {}),
      )
      .into()
  });
  for id in ["plain", "bounded"] {
    let (frame, label) = (kit.bounds(id), kit.bounds(&format!("{id}-label")));
    assert_close(frame.width, 300.0, &format!("{id} is as wide as its parent"));
    assert_close(label.x - frame.x, 12.0, &format!("{id} label at the start"));
    assert!(
      label.width > frame.width / 2.0,
      "{id} label fills the room up to the chevron: {}",
      label.width
    );
  }
}

/// A title bar's control keeps its press from the bar, which hands a press it receives to the OS to move the
/// window (lurq's `ChromeTitleBar`), and the OS move loop takes the release, so no click would follow. The
/// bar's empty room still takes the press.
#[test]
fn a_title_bar_control_keeps_its_press_from_the_bars_window_move() {
  let (moves, mv) = counter();
  let (presses, press) = counter();
  let (plain_presses, plain_press) = counter();
  let mut kit = Kit::show(move |_| {
    let mv = mv.clone();
    Row::new()
      .id("bar")
      .on_mouse_down(move |event: MouseEvent| {
        if event.button == MouseButton::Left {
          mv();
        }
      })
      .child(
        in_title_bar(pressable(press.clone()))
          .id("control")
          .width(Dimension::Px(60.0))
          .height(Dimension::Px(28.0)),
      )
      .child(
        pressable(plain_press.clone())
          .id("plain")
          .width(Dimension::Px(60.0))
          .height(Dimension::Px(28.0)),
      )
      .child(
        Row::new()
          .id("room")
          .width(Dimension::Px(200.0))
          .height(Dimension::Px(28.0)),
      )
      .into()
  });
  kit.click("control");
  assert_eq!(presses.load(Ordering::SeqCst), 1, "the control is pressed");
  assert_eq!(moves.load(Ordering::SeqCst), 0, "and the bar does not move the window");
  kit.click("plain");
  assert_eq!(plain_presses.load(Ordering::SeqCst), 1);
  assert_eq!(
    moves.load(Ordering::SeqCst),
    1,
    "a press a control lets through reaches the bar"
  );
  kit.click("room");
  assert_eq!(moves.load(Ordering::SeqCst), 2, "the bar's empty room moves the window");
}
