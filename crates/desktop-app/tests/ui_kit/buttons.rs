//! Buttons, icon buttons and unavailable actions.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  app::theme::{RadiusSize, SpacingSize},
  components::Row,
  node::Element,
};
use orchester_app::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  icon_button::IconButton,
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
          .unavailable(),
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
