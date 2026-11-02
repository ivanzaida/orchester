//! The context meter (ORC-305, ORC-308): its four states in a composer's bar and on a divider, its tooltip and its
//! popover with the tokens used of the window, an estimate marked as one, and a reading that changes while it shows;
//! *Compact* beside it and in its popover, waiting while a turn is under way, and the meter while it compacts
//! (ORC-309).

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::{
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_app::ui::{
  context_meter::{Compact, ContextMeterProps, MeterPlace, MeterReading, Over, context_meter},
  icon::Icon,
};
use orchester_core::connection::context_use::ContextUse;

use crate::harness::{Kit, assert_close};

fn props(id: &str, reading: Option<ContextUse>, place: MeterPlace) -> ContextMeterProps {
  ContextMeterProps {
    id: id.into(),
    reading,
    agent: None,
    model: Some("claude-opus-5-5".into()),
    place,
    over: None,
    compact: Compact::NotOffered,
  }
}

#[test]
fn context_meter_popover_opens_above_the_composer_box_at_its_right_edge() {
  let mut kit = Kit::show(|ctx| {
    let composer = ElementRef::new();
    let meter = context_meter(
      ctx,
      ContextMeterProps {
        over: Some(Over(composer.clone())),
        ..props("near", Some(ContextUse::reported(172_000, 200_000)), MeterPlace::Bar)
      },
    );
    let bar = Row::new()
      .width(Dimension::Pct(100.0))
      .child(Spacer::new().flex(1.0))
      .child(meter)
      .child(Text::new("Ctrl Enter").id("keys"));
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(Dimension::Px(400.0))
      .child(
        Column::new()
          .id("box")
          .ref_element(composer)
          .width(Dimension::Px(760.0))
          .padding(Padding::all(Dimension::Px(16.0)))
          .child(Text::new("Message the Orchestrator…"))
          .child(bar),
      )
      .into()
  });
  kit.click("near");
  let (popover, composer) = (kit.bounds("near-popover"), kit.bounds("box"));
  assert_close(
    popover.x + popover.width,
    composer.x + composer.width,
    "its right edge on the box's",
  );
  assert_close(
    popover.y + popover.height,
    composer.y - 6.0,
    "its bottom 6 over the box's top",
  );
  kit.click("near");
  assert!(!kit.exists("near-popover"), "the meter closes it again");
}

#[test]
fn context_meter_offers_compact_beside_it_from_near_full_and_in_its_popover() {
  let pressed = Arc::new(AtomicUsize::new(0));
  let counted = pressed.clone();
  let mut kit = Kit::show(move |ctx| {
    let counted = counted.clone();
    let ready = Compact::Ready(Arc::new(move || {
      counted.fetch_add(1, Ordering::SeqCst);
    }));
    let offered = |id, reading, compact: &Compact| ContextMeterProps {
      compact: compact.clone(),
      ..props(id, Some(reading), MeterPlace::Bar)
    };
    Column::new()
      .child(context_meter(
        ctx,
        offered("plenty", ContextUse::reported(76_000, 200_000), &ready),
      ))
      .child(context_meter(
        ctx,
        offered("near", ContextUse::reported(172_000, 200_000), &ready),
      ))
      .child(context_meter(
        ctx,
        offered("busy", ContextUse::reported(172_000, 200_000), &Compact::Waits),
      ))
      .child(context_meter(
        ctx,
        offered("running", ContextUse::reported(172_000, 200_000), &Compact::Running),
      ))
      .into()
  });
  assert!(
    !kit.exists("plenty-compact"),
    "under 80% Compact is in the popover only"
  );
  assert!(kit.exists("near-compact"), "from 80% it sits beside the meter");
  kit.click("near-compact");
  assert_eq!(pressed.load(Ordering::SeqCst), 1, "a press compacts");
  kit.click("plenty");
  assert!(kit.texts("plenty-popover-hint")[0].starts_with("Compact replaces"));
  kit.click("plenty-popover-compact");
  assert_eq!(pressed.load(Ordering::SeqCst), 2, "the popover's Compact compacts");
  assert!(!kit.exists("plenty-popover"), "and closes the popover");
  kit.click("busy-compact");
  assert_eq!(pressed.load(Ordering::SeqCst), 2, "it waits while a turn is under way");
  assert_eq!(
    kit.texts("running-share"),
    ["Compacting…"],
    "the meter while it compacts"
  );
  assert!(!kit.exists("running-compact"), "and no Compact then");
}

/// A window showing one meter per reading, in a composer's bar.
fn meters(readings: Vec<(&'static str, Option<ContextUse>)>) -> Kit {
  Kit::show(move |ctx| {
    let mut column = Column::new();
    for (id, reading) in &readings {
      column = column.child(context_meter(ctx, props(id, *reading, MeterPlace::Bar)));
    }
    column.into()
  })
}

#[test]
fn context_meter_states_read_plenty_near_full_full_and_unknown() {
  let mut kit = meters(vec![
    ("plenty", Some(ContextUse::reported(76_000, 200_000))),
    ("near", Some(ContextUse::reported(172_000, 200_000))),
    ("full", Some(ContextUse::reported(200_000, 200_000))),
    ("unknown", None),
    ("no-window", Some(ContextUse::unknown_window(720))),
  ]);
  assert_eq!(kit.texts("plenty-share"), ["38%"]);
  assert_eq!(kit.texts("near-share"), ["86%"], "from 80% it is near full");
  assert_eq!(
    kit.texts("full-share"),
    ["Full"],
    "the whole window reads Full, not 100%"
  );
  for ring in ["plenty-ring", "near-ring", "full-ring"] {
    assert!(kit.exists(ring), "{ring} is drawn");
  }
  for unknown in ["unknown", "no-window"] {
    assert_eq!(
      kit.texts(&format!("{unknown}-ring")),
      [Icon::CircleDashed.glyph().to_string()],
      "{unknown}: a dashed ring"
    );
    assert!(!kit.exists(&format!("{unknown}-share")), "{unknown}: no share");
  }
  assert_close(kit.bounds("plenty").height, 24.0, "the bar's hit area");
  assert_close(kit.bounds("plenty-ring").width, 12.0, "the ring");
}

#[test]
fn context_meter_tooltip_says_the_share_and_tokens() {
  let mut kit = meters(vec![
    ("plenty", Some(ContextUse::reported(76_000, 200_000))),
    ("full", Some(ContextUse::reported(200_000, 200_000))),
    ("estimate", Some(ContextUse::new(108_000, 128_000, true))),
    ("unknown", None),
  ]);
  let cases = [
    ("plenty", "Context 38% · 76k of 200k"),
    ("full", "Context full · 200k of 200k"),
    ("estimate", "Context ≈ 84% · 108k of 128k"),
    ("unknown", "Context size not reported"),
  ];
  for (id, words) in cases {
    kit.rest_on(id);
    assert_eq!(kit.texts(&format!("{id}-tooltip")), [words], "{id}");
  }
}

#[test]
fn context_meter_popover_shows_used_of_window_and_marks_an_estimate() {
  let mut kit = meters(vec![
    ("near", Some(ContextUse::reported(172_000, 200_000))),
    ("estimate", Some(ContextUse::new(108_000, 128_000, true))),
  ]);
  kit.click("near");
  assert_eq!(kit.texts("near-popover-title"), ["Context"]);
  assert_eq!(kit.texts("near-popover-badge-label"), ["Near full"]);
  assert_eq!(kit.texts("near-popover-model"), ["claude-opus-5-5"]);
  assert_eq!(kit.texts("near-popover-tokens"), ["172k of 200k tokens"]);
  assert_eq!(kit.texts("near-popover-share"), ["86%"]);
  assert!(kit.exists("near-popover-bar"), "the share's bar");
  assert!(!kit.exists("near-popover-note"), "a reported window needs no note");
  assert!(
    !kit.exists("near-popover-compact"),
    "Compact is offered only where the session can be compacted"
  );
  kit.click("near");
  kit.click("estimate");
  assert_eq!(kit.texts("estimate-popover-tokens"), ["≈ 108k of 128k tokens"]);
  assert_eq!(kit.texts("estimate-popover-share"), ["≈ 84%"]);
  assert!(
    kit.texts("estimate-popover-note")[0].starts_with("An estimate"),
    "an estimate says it is one"
  );
}

#[test]
fn context_meter_popover_of_a_full_and_an_unknown_window() {
  let mut kit = meters(vec![
    ("full", Some(ContextUse::reported(210_000, 200_000))),
    ("unknown", None),
    ("no-window", Some(ContextUse::unknown_window(720))),
  ]);
  kit.click("full");
  assert_eq!(kit.texts("full-popover-badge-label"), ["Full"]);
  assert_eq!(kit.texts("full-popover-tokens"), ["210k of 200k tokens"]);
  kit.click("full");
  kit.click("unknown");
  assert!(!kit.exists("unknown-popover-tokens"), "nothing counted");
  assert!(!kit.exists("unknown-popover-bar"), "no share, no bar");
  assert!(kit.texts("unknown-popover-note")[0].contains("no share to show"));
  kit.click("unknown");
  kit.click("no-window");
  assert_eq!(kit.texts("no-window-popover-tokens"), ["720 tokens"]);
  assert!(!kit.exists("no-window-popover-bar"));
}

#[test]
fn context_meter_follows_each_new_reading_with_its_popover_open() {
  let reading = Signal::new(MeterReading(Some(ContextUse::reported(60_000, 200_000))));
  let shown = reading.clone();
  let mut kit = Kit::show(move |ctx| -> Element {
    let props = ContextMeterProps {
      agent: Some("Builder".into()),
      ..props("step", shown.get().0, MeterPlace::Divider)
    };
    context_meter(ctx, props)
  });
  assert_eq!(kit.texts("step-share"), ["30%"]);
  assert_close(
    kit.bounds("step").height,
    kit.bounds("step-ring").height.max(kit.bounds("step-share").height),
    "a divider's meter has no hit area padding",
  );
  kit.click("step");
  assert_eq!(kit.texts("step-popover-model"), ["Builder · claude-opus-5-5"]);
  reading.set(MeterReading(Some(ContextUse::reported(170_000, 200_000))));
  kit.pass();
  assert_eq!(kit.texts("step-share"), ["85%"], "a new turn's reading");
  assert_eq!(
    kit.texts("step-popover-tokens"),
    ["170k of 200k tokens"],
    "the open popover follows it"
  );
  assert_eq!(kit.texts("step-popover-badge-label"), ["Near full"]);
  reading.set(MeterReading(Some(ContextUse::reported(200_000, 200_000))));
  kit.pass();
  assert_eq!(kit.texts("step-share"), ["Full"]);
}
