//! Checkboxes, radios, switches, page tabs and segmented controls.

use std::sync::Arc;

use lurq::{app::theme::SpacingSize, components::Row, core::Signal};
use orchester_app::ui::{
  icon::Icon,
  pressable::Focus,
  tabs::{PageTab, Segment, SegmentFace, counted, page_tabs, segmented},
  toggle::{Toggle, ToggleKind},
};
use parking_lot::Mutex;

use crate::harness::{Kit, assert_close, gap_after};

#[test]
fn toggles_are_their_design_size_8_from_their_label_and_ask_for_the_other_value() {
  let asked: Arc<Mutex<Vec<(&str, bool)>>> = Arc::default();
  let heard = asked.clone();
  let mut kit = Kit::show(move |_| {
    let hear = |name: &'static str| {
      let heard = heard.clone();
      move |value| heard.lock().push((name, value))
    };
    Row::new()
      .spacing(SpacingSize::Lg)
      .child(Toggle::new("check", ToggleKind::Checkbox, false, "Also save").on_change(hear("check")))
      .child(Toggle::new("radio", ToggleKind::Radio, true, "Selected").on_change(hear("radio")))
      .child(Toggle::new("switch", ToggleKind::Switch, false, "Network").on_change(hear("switch")))
      .child(Toggle::new("on", ToggleKind::Switch, true, "Ask before MCP actions").on_change(hear("on")))
      .child(Toggle::new("fixed", ToggleKind::Checkbox, true, "From the profile"))
      .into()
  });
  for (id, size) in [
    ("check", (16.0, 16.0)),
    ("radio", (16.0, 16.0)),
    ("switch", (28.0, 16.0)),
  ] {
    let mark = kit.bounds(&format!("{id}-mark"));
    assert_eq!((mark.width, mark.height), size, "{id}");
  }
  assert_close(
    gap_after(kit.bounds("check-mark"), kit.bounds("check-label")),
    8.0,
    "mark to label",
  );
  let (off, on) = (kit.bounds("switch-mark"), kit.bounds("on-mark"));
  let (off_knob, on_knob) = (kit.bounds("switch-knob"), kit.bounds("on-knob"));
  assert_eq!((off_knob.width, off_knob.height), (12.0, 12.0));
  assert_close(off_knob.x - off.x, 2.0, "an off knob at the start");
  assert_close(
    on.x + on.width - on_knob.x - on_knob.width,
    2.0,
    "an on knob at the end",
  );
  for id in ["check", "radio", "switch", "on", "fixed"] {
    kit.click(id);
  }
  assert_eq!(
    *asked.lock(),
    [("check", true), ("radio", true), ("switch", true), ("on", false)],
    "a fixed control cannot be flipped"
  );
}

#[test]
fn page_tabs_are_34_high_20_apart_with_counts_and_a_locked_tab_that_cannot_open() {
  let opened = Signal::new(Vec::<&str>::new());
  let heard = opened.clone();
  let mut kit = Kit::show(move |ctx| {
    let open = |name: &'static str| {
      let heard = heard.clone();
      move || heard.update(|opened| opened.push(name))
    };
    let tabs = [
      PageTab::new("tab-all", counted(ctx, "All", 24), true, open("all")),
      PageTab::new("tab-review", counted(ctx, "In review", 2), false, open("review")),
      PageTab::unavailable("tab-artifacts", "Artifacts", "Artifacts are planned."),
    ];
    page_tabs(ctx, "tabs", tabs).into()
  });
  let (all, review, artifacts) = (
    kit.bounds("tab-all"),
    kit.bounds("tab-review"),
    kit.bounds("tab-artifacts"),
  );
  assert_eq!(all.height, 34.0);
  let (tabs, line) = (kit.bounds("tabs"), kit.bounds("tabs-line"));
  assert_eq!(
    (line.x, line.y, line.width, line.height),
    (tabs.x, all.y + 33.0, tabs.width, 1.0),
    "the line under the tabs, under the current tab's underline"
  );
  assert_close(gap_after(all, review), 20.0, "between tabs");
  assert_close(gap_after(review, artifacts), 20.0, "before the locked tab");
  assert_close(kit.bounds("tab-all-label").x - all.x, 2.0, "a tab's side");
  assert_eq!(kit.texts("tab-all"), ["All 24"]);
  assert_eq!(
    kit.texts("tab-artifacts"),
    ["Artifacts".to_owned(), Icon::Lock.glyph().to_string()],
    "the label, then the lock (`Smb4t`)"
  );
  assert_close(
    gap_after(kit.bounds("tab-artifacts-label"), kit.bounds("tab-artifacts-icon")),
    5.0,
    "label to lock",
  );
  kit.click("tab-review");
  kit.click("tab-artifacts");
  assert_eq!(opened.get(), ["review"]);
  kit.rest_on("tab-artifacts");
  assert_eq!(
    kit.texts("tab-artifacts-tooltip"),
    ["Artifacts are planned."],
    "why, on hover"
  );
}

#[test]
fn a_segmented_control_is_26_high_2_inside_its_segments_22_high() {
  let chosen = Signal::new("dark");
  let shown = chosen.clone();
  let mut kit = Kit::show(move |_| {
    let segment = |id: &'static str, label: &'static str| {
      let chosen = shown.clone();
      let set = chosen.clone();
      Segment::new(id, SegmentFace::Text(label.into()), chosen.get() == id, move || {
        set.set(id)
      })
    };
    let view = |id: &'static str, glyph, label: &'static str| {
      Segment::new(id, SegmentFace::Icon(glyph, label.into()), id == "grid", || {})
    };
    Row::new()
      .spacing(SpacingSize::Lg)
      .child(segmented(
        "mode",
        [
          segment("dark", "Dark"),
          segment("light", "Light"),
          segment("system", "System"),
        ],
        Focus::Form,
      ))
      .child(segmented(
        "view",
        [view("grid", Icon::LayoutGrid, "Grid"), view("list", Icon::List, "List")],
        Focus::Pointer,
      ))
      .into()
  });
  let (control, dark, light) = (kit.bounds("mode"), kit.bounds("dark"), kit.bounds("light"));
  assert_eq!(control.height, 26.0);
  assert_eq!(dark.height, 22.0);
  assert_close(dark.x - control.x, 2.0, "inside");
  assert_close(gap_after(dark, light), 2.0, "between segments");
  assert_close(kit.bounds("dark-label").x - dark.x, 10.0, "a text segment's side");
  let grid = kit.bounds("grid");
  assert_eq!((grid.width, kit.bounds("view").height), (28.0, 26.0));
  kit.click("light");
  assert_eq!(chosen.get(), "light");
}
