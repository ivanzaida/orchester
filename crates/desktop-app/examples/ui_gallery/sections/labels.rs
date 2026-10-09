//! Checkboxes, radios and switches (`Checkbox/*`, `Radio/*` `RchTU`, `Switch/*`), page tabs with counts
//! and a locked tab (`Page tab/*`, `W4QYDM`), segmented controls (`Segmented` `C9VsTA`, `S5Fsve`),
//! status, priority and workflow labels (`Status` `VWRbG`, `Run line` `AgVPe`, `Priority cell`
//! `OfuTC`), chips and badges (`Chip` `jI8af`, `Chip/removable` `qj3Ss`, `Badge` `Fm7lc`), avatars,
//! breadcrumbs and detail rows.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::Element,
};
use orchester_app::ui::{
  avatar::{agent_avatar, initials, person_avatar},
  breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
  chip::{BadgeTone, TagChip, badge},
  detail_row::{DetailValue, detail_row},
  icon::Icon,
  pressable::Focus,
  status::{Status, StatusChip, priority_chip, workflow_chip},
  tabs::{PageTab, Segment, SegmentFace, counted, page_tabs, segmented},
  toggle::{Toggle, ToggleKind},
};
use orchester_core::task::organization::{Priority, WorkflowStatus};

use super::{cell, line, stack, wide_cell};
use crate::gallery::State;

/// The tabs' cell, as wide as the Tasks screen's content at 1440.
const TABS: f32 = 720.0;
const DETAILS: f32 = 340.0;

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  stack([
    line(toggles(state)).into(),
    line([
      wide_cell("Page tab/* · loUWZ, NspMt, W4QYDM", TABS, tabs(ctx, state)),
      cell("Segmented/text · S5Fsve", segments(state)),
      cell("Segmented · C9VsTA", views(state)),
    ])
    .into(),
    line([cell("Status · VWRbG, every word", statuses(ctx))]).into(),
    line([
      cell("compact · Run line AgVPe", compact(ctx)),
      cell("Priority cell · OfuTC", priorities(ctx)),
      cell("workflow", workflows(ctx)),
    ])
    .into(),
    line([
      cell("Chip · jI8af, Chip/removable · qj3Ss", chips(state)),
      cell("Badge · Fm7lc", badges()),
      cell("Avatar · x5rV0, HO4bJ", avatars()),
    ])
    .into(),
    line([
      cell(
        "Crumbs · A7q3o (task page)",
        crumbs("g-crumbs-task", CrumbSeparator::Slash, state),
      ),
      cell(
        "Crumbs · VpX0E (New task)",
        crumbs("g-crumbs-new", CrumbSeparator::Chevron, state),
      ),
      wide_cell("Detail row · paUqV", DETAILS, details()),
    ])
    .into(),
  ])
}

fn toggles(state: &State) -> [Column; 3] {
  let checked = state.checked.get();
  let check = state.checked.clone();
  let checkboxes = Column::new()
    .spacing(SpacingSize::Sm)
    .child(
      Toggle::new("g-checkbox", ToggleKind::Checkbox, checked)
        .label("Also save to the profile")
        .on_change(move |on| check.set(on)),
    )
    .child(Toggle::new("g-checkbox-fixed", ToggleKind::Checkbox, true).label("From the profile · fixed"));
  let chosen = state.radio.get();
  let radio = |id: &'static str, value: &'static str, label: &'static str| {
    let set = state.radio.clone();
    Toggle::new(id, ToggleKind::Radio, chosen == value)
      .label(label)
      .on_change(move |_| set.set(value))
  };
  let radios = Column::new()
    .spacing(SpacingSize::Sm)
    .child(radio("g-radio-edit", "edit", "Edit files in the task worktree"))
    .child(radio("g-radio-read", "read", "Read-only"));
  let switched = state.switched.get();
  let flip = state.switched.clone();
  let switches = Row::new()
    .spacing(SpacingSize::Lg)
    .child(
      Toggle::new("g-switch", ToggleKind::Switch, switched)
        .label("Network")
        .on_change(move |on| flip.set(on)),
    )
    .child(Toggle::new("g-switch-fixed", ToggleKind::Switch, true).label("Fixed"));
  [
    cell("Checkbox/* · q3JN1, Una0j", checkboxes),
    cell("Radio/* · RchTU, vsbEb", radios),
    cell("Switch/* · H6LlDD, XC5zU", switches),
  ]
}

fn tabs(ctx: &Ctx, state: &State) -> Row {
  let current = state.tab.get();
  let tab = |id: &'static str, label: &str, count: usize| {
    let set = state.tab.clone();
    PageTab::new(
      format!("g-tab-{id}"),
      counted(ctx, label, count),
      current == id,
      move || set.set(id),
    )
  };
  page_tabs(
    "g-tabs",
    [
      tab("all", "All", 24),
      tab("needs", "Needs you", 4),
      tab("review", "In review", 2),
      PageTab::unavailable("g-tab-artifacts", "Artifacts"),
    ],
  )
}

fn segments(state: &State) -> Row {
  let current = state.segment.get();
  let segment = |id: &'static str, label: &'static str| {
    let set = state.segment.clone();
    Segment::new(
      format!("g-segment-{id}"),
      SegmentFace::Text(label.into()),
      current == id,
      move || set.set(id),
    )
  };
  segmented(
    "g-segmented",
    [
      segment("all", "All"),
      segment("messages", "Messages"),
      segment("tools", "Tools"),
    ],
    Focus::Form,
  )
}

fn views(state: &State) -> Row {
  let current = state.segment.get();
  let view = |id: &'static str, glyph: Icon, label: &'static str| {
    let set = state.segment.clone();
    Segment::new(
      format!("g-view-{id}"),
      SegmentFace::Icon(glyph, label.into()),
      current == id,
      move || set.set(id),
    )
  };
  segmented(
    "g-views",
    [view("all", Icon::LayoutGrid, "Grid"), view("tools", Icon::List, "List")],
    Focus::Pointer,
  )
}

fn statuses(ctx: &Ctx) -> Column {
  Status::ALL
    .chunks(7)
    .enumerate()
    .fold(Column::new().spacing(SpacingSize::Md), |column, (index, chunk)| {
      column.child(
        chunk
          .iter()
          .enumerate()
          .fold(Row::new().spacing(SpacingSize::Xl), |row, (place, status)| {
            row.child(StatusChip::new(format!("g-status-{}", index * 7 + place), *status).render(ctx))
          }),
      )
    })
}

fn compact(ctx: &Ctx) -> Column {
  Column::new()
    .spacing(SpacingSize::Sm)
    .child(
      StatusChip::new("g-run-running", Status::Running)
        .detail("02 Plan")
        .compact()
        .render(ctx),
    )
    .child(StatusChip::new("g-run-needs", Status::NeedsInput).compact().render(ctx))
    .child(
      StatusChip::new("g-run-failed", Status::Failed)
        .detail("04 Run checks")
        .compact()
        .render(ctx),
    )
}

fn priorities(ctx: &Ctx) -> Column {
  Priority::ALL
    .iter()
    .enumerate()
    .fold(Column::new().spacing(SpacingSize::Sm), |column, (index, priority)| {
      column.child(priority_chip(ctx, &format!("g-priority-{index}"), *priority))
    })
}

fn workflows(ctx: &Ctx) -> Column {
  WorkflowStatus::ALL
    .iter()
    .enumerate()
    .fold(Column::new().spacing(SpacingSize::Sm), |column, (index, status)| {
      column.child(workflow_chip(ctx, &format!("g-workflow-{index}"), *status))
    })
}

fn chips(state: &State) -> Row {
  let pressed = state.pressed.clone();
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(TagChip::new("g-tag", "webhooks"))
    .child(
      TagChip::new("g-tag-removable", "backend")
        .removable("Remove backend", move || pressed.set("Remove backend".to_owned())),
    )
}

fn badges() -> Row {
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(badge("g-badge-success", BadgeTone::Success, "Saved"))
    .child(badge("g-badge-info", BadgeTone::Info, "New"))
    .child(badge("g-badge-warning", BadgeTone::Warning, "Stale"))
    .child(badge("g-badge-danger", BadgeTone::Danger, "Failed"))
}

fn avatars() -> Row {
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(person_avatar("g-avatar", &initials("Ada Lovelace")))
    .child(agent_avatar("g-agent", Icon::Hammer))
}

fn crumbs(id: &str, separator: CrumbSeparator, state: &State) -> Row {
  let link = |label: &'static str| {
    let pressed = state.pressed.clone();
    Crumb::link(label, move || pressed.set(label.to_owned()))
  };
  let mut crumbs = Vec::new();
  if separator == CrumbSeparator::Slash {
    crumbs.push(link("Kontur"));
  }
  crumbs.push(link("Tasks"));
  crumbs.push(match separator {
    CrumbSeparator::Slash => Crumb::here("KON-12").mono(),
    CrumbSeparator::Chevron => Crumb::here("New task"),
  });
  breadcrumb(id, separator, crumbs)
}

fn details() -> Column {
  Column::new()
    .child(detail_row(
      "g-detail-pipeline",
      "Pipeline",
      "Feature delivery v3",
      DetailValue::Text,
    ))
    .child(detail_row(
      "g-detail-branch",
      "Branch",
      "kon-12-webhook-retry",
      DetailValue::Mono,
    ))
    .child(detail_row(
      "g-detail-worktree",
      "Worktree",
      "2 files not committed after 7c1e9f2",
      DetailValue::Text,
    ))
}
