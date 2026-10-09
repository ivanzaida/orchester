//! Status, priority and workflow labels, chips, badges, avatars, breadcrumbs and detail rows.

use std::collections::BTreeSet;

use lurq::{
  app::theme::{PaletteColor, SpacingSize},
  components::{Column, Row},
  node::Element,
};
use orchester_app::ui::{
  avatar::{agent_avatar, initials, person_avatar},
  breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
  chip::{BadgeTone, TagChip, badge},
  detail_row::{DetailValue, detail_row},
  icon::Icon,
  status::{Status, StatusChip, priority_chip, workflow_chip},
};
use orchester_core::{
  run::TaskPhase,
  task::organization::{Priority, WorkflowStatus},
};

use crate::harness::{Kit, assert_close, gap_after};

#[test]
fn a_status_is_its_icon_and_word_5_apart_in_its_tone() {
  let mut kit = Kit::show(|ctx| {
    Column::new()
      .spacing(SpacingSize::Sm)
      .child(
        StatusChip::new("running", Status::Running)
          .detail("03 Implement")
          .render(ctx),
      )
      .child(StatusChip::new("needs", Status::NeedsInput).compact().render(ctx))
      .into()
  });
  assert_eq!(
    kit.texts("running"),
    [
      Icon::LoaderCircle.glyph().to_string(),
      "Running · 03 Implement".to_owned()
    ]
  );
  let (glyph, word) = (kit.bounds("running-icon"), kit.bounds("running-label"));
  assert_close(gap_after(glyph, word), 5.0, "icon to word");
  assert_close(glyph.height, 13.0, "regular icon");
  assert_close(kit.bounds("needs-icon").height, 12.0, "compact icon");
  assert_eq!(kit.texts("needs")[1], "Needs input");
}

#[test]
fn every_status_has_an_icon_and_a_word_and_failures_read_at_a_glance() {
  let kit = Kit::show(|_| Column::new().into());
  let i18n = kit.app.i18n();
  let mut words = BTreeSet::new();
  for status in Status::ALL {
    let word = i18n.t(status.key());
    assert_ne!(&*word, status.key(), "{status:?} has its word in en.json");
    assert!(words.insert(word.to_string()), "{status:?}'s word is its own");
  }
  for status in [Status::Failed, Status::Unreadable] {
    assert_eq!(status.color(), PaletteColor::Danger, "{status:?}");
  }
  assert_ne!(
    Status::Paused.icon(),
    Status::StoppedAtLimit.icon(),
    "a limit stop is not a pause"
  );
  assert_eq!(Status::of_phase(TaskPhase::Waiting), Status::NeedsInput);
  assert_eq!(Status::of_phase(TaskPhase::Succeeded), Status::Completed);
  assert_eq!(Status::of_phase(TaskPhase::Cancelled), Status::Canceled);
}

#[test]
fn priorities_and_workflow_statuses_read_as_icon_and_word() {
  let mut kit = Kit::show(|ctx| {
    Column::new()
      .child(priority_chip(ctx, "high", Priority::High))
      .child(priority_chip(ctx, "none", Priority::None))
      .child(workflow_chip(ctx, "review", WorkflowStatus::InReview))
      .into()
  });
  assert_eq!(
    kit.texts("high"),
    [Icon::SignalHigh.glyph().to_string(), "High".to_owned()]
  );
  assert_eq!(
    kit.texts("none"),
    [Icon::Minus.glyph().to_string(), "No priority".to_owned()]
  );
  assert_eq!(
    kit.texts("review"),
    [Icon::Eye.glyph().to_string(), "In review".to_owned()]
  );
  assert_close(
    gap_after(kit.bounds("high-icon"), kit.bounds("high-label")),
    5.0,
    "icon to word",
  );
}

#[test]
fn a_tag_is_a_pill_2_over_and_8_beside_its_name_and_a_badge_3_and_8() {
  let mut kit = Kit::show(|_| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(TagChip::new("tag", "webhooks"))
      .child(TagChip::new("removable", "backend").removable("Remove backend", || {}))
      .child(badge("saved", BadgeTone::Success, "Saved"))
      .into()
  });
  let (chip, name) = (kit.bounds("tag"), kit.bounds("tag-label"));
  assert_close(name.x - chip.x, 8.0, "tag start");
  assert_close(name.y - chip.y, 2.0, "tag top");
  assert_close(chip.x + chip.width - name.x - name.width, 8.0, "tag end");
  let (removable, cross) = (kit.bounds("removable"), kit.bounds("removable-remove"));
  assert_close(
    removable.x + removable.width - cross.x - cross.width,
    4.0,
    "the cross 4 from the end",
  );
  assert_close(
    gap_after(kit.bounds("removable-label"), cross),
    4.0,
    "the cross 4 after the name",
  );
  let (pill, dot, word) = (kit.bounds("saved"), kit.bounds("saved-dot"), kit.bounds("saved-label"));
  assert_close(dot.x - pill.x, 8.0, "badge start");
  assert_close(word.y - pill.y, 3.0, "badge top");
  assert_close(gap_after(dot, word), 6.0, "dot to word");
  assert_eq!((dot.width, dot.height), (6.0, 6.0));
}

#[test]
fn avatars_breadcrumbs_and_detail_rows_take_their_design_sizes() {
  let mut kit = Kit::show(|ctx| {
    let crumbs = breadcrumb(
      ctx,
      "crumbs",
      CrumbSeparator::Slash,
      vec![
        Crumb::link("Kontur", || {}),
        Crumb::link("Tasks", || {}),
        Crumb::here("KON-12").mono(),
      ],
    );
    Column::new()
      .spacing(SpacingSize::Sm)
      .child(person_avatar("person", &initials("Ada Lovelace")))
      .child(agent_avatar("agent", Icon::Hammer))
      .child(crumbs)
      .child(Element::from(
        Row::new()
          .width(lurq::node::dimension::Dimension::Px(300.0))
          .child(detail_row(
            "branch",
            "Branch",
            "kon-12-webhook-retry",
            DetailValue::Mono,
          )),
      ))
      .into()
  });
  let (person, agent) = (kit.bounds("person"), kit.bounds("agent"));
  assert_eq!((person.width, person.height), (22.0, 22.0));
  assert_eq!((agent.width, agent.height), (24.0, 24.0));
  assert_eq!(kit.texts("person"), ["AL"]);
  assert_eq!(kit.texts("crumbs"), ["Kontur", "/", "Tasks", "/", "KON-12"]);
  let (first, slash, second) = (
    kit.bounds("crumbs-0"),
    kit.bounds("crumbs-separator-1"),
    kit.bounds("crumbs-1"),
  );
  assert_close(gap_after(first, slash), 6.0, "crumb to separator");
  assert_close(gap_after(slash, second), 6.0, "separator to crumb");
  let (key, value) = (kit.bounds("branch-key"), kit.bounds("branch"));
  assert_eq!(key.width, 108.0, "the key column");
  assert_close(gap_after(key, value), 12.0, "key to value");
}
