//! The grouped list of records the Memory screens share (`Record list` `sglU2` in `a0PLO`, `Fqhrb` in `TeKGM`): the
//! records waiting for approval first on a task's tab, then a group per type under its heading; in a record's place
//! its editor or its delete confirmation while one is open, and on the project's page its details under its row
//! while a press opened them (`aBtsx`). A task's handoffs show the latest, with *Show N earlier handoffs* for the
//! steps' other ones.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::MemoryId,
  memory::{Memory, MemoryKind, MemoryStatus},
};
use orchester_workspace::memory::{
  listing::{self, Filters, Group, Listed},
  origin::Origins,
  read::ProjectMemory,
};

use super::{
  delete::DeleteConfirm,
  details::{DetailsProps, RecordDetails},
  editor::{RecordEditor, opened_on},
  opened::{self, Among},
  parts,
  proposal::{self, Listed as Place},
  row::{Details, Earlier, RecordRow, RowProps},
  state::{MemoryState, Opened},
  words,
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, MemorySpacing},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// What a list shows, and how.
#[derive(Clone, Copy)]
pub struct ListInput<'a> {
  pub records: &'a [Memory],
  pub origins: &'a Origins,
  pub filters: &'a Filters,
  /// The task's tab: the waiting records first, and the latest handoff flagged.
  pub task_tab: bool,
  /// The project's page, whose records open their details in place.
  pub project: Option<&'a Arc<ProjectMemory>>,
}

impl<'a> ListInput<'a> {
  const fn among(&self) -> Among<'a> {
    Among {
      records: self.records,
      origins: self.origins,
      with_task: !self.task_tab,
      project: None,
    }
  }
}

/// The groups of `input`'s records, each its heading and its records.
pub fn groups(ctx: &mut Ctx, state: &MemoryState, input: ListInput<'_>) -> Vec<Element> {
  let latest = input.task_tab.then(|| listing::latest_handoff(input.records)).flatten();
  listing::grouped(input.records, input.filters, input.task_tab)
    .into_iter()
    .map(|listed| group(ctx, state, input, (&listed, latest)))
    .collect()
}

/// `input`'s records in one list without group headings, as the project page's *Deprecated*, *Superseded* and
/// *Rejected* tabs show them (`hxQhj` in `C0OJpC`).
pub fn flat(ctx: &mut Ctx, state: &MemoryState, input: ListInput<'_>) -> Vec<Element> {
  listing::flat(input.records, input.filters)
    .into_iter()
    .map(|memory| entry(ctx, state, input, (memory, None)))
    .collect()
}

fn group(
  ctx: &mut Ctx,
  state: &MemoryState,
  input: ListInput<'_>,
  (listed, latest): (&Listed<'_>, Option<MemoryId>),
) -> Element {
  let (key, label) = match listed.group {
    Group::Waiting => ("waiting".to_owned(), ctx.t("memory.group.waiting")),
    Group::Kind(kind) => (
      kind.name().to_owned(),
      words::kind_plural(ctx, kind).to_uppercase().into(),
    ),
  };
  let id = format!("memory-group-{key}");
  let heading = parts::group_heading(&format!("{id}-heading"), &label, listed.records.len());
  let mut shown: Vec<&Memory> = listed.records.clone();
  let mut earlier: Vec<&Memory> = Vec::new();
  if listed.group == Group::Kind(MemoryKind::Handoff) && latest.is_some() && !state.earlier_handoffs.get() {
    (shown, earlier) = listed
      .records
      .iter()
      .partition(|memory| Some(memory.id()) == latest || memory.status() != MemoryStatus::Active);
  }
  let mut column = Column::new().id(id).width(Dimension::Pct(100.0));
  column = if input.task_tab {
    column.spacing(SpacingSize::Xs).child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(SpacingSize::Md))
        .child(heading),
    )
  } else {
    column.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(crate::theme::ExtraSpacing::Inset.role())
            .bottom(SpacingSize::Xs)
            .left(SpacingSize::Md)
            .right(SpacingSize::Md),
        )
        .child(heading),
    )
  };
  for memory in shown {
    column = column.child(entry(ctx, state, input, (memory, latest)));
  }
  if listed.group == Group::Kind(MemoryKind::Handoff) && (!earlier.is_empty() || state.earlier_handoffs.get()) {
    column = column.child(earlier_handoffs(ctx, state, input, &earlier));
  }
  column.into()
}

/// A record in its list: its editor or its delete confirmation while one is open in its place, else its row, with
/// its details under it while they are open.
fn entry(
  ctx: &mut Ctx,
  state: &MemoryState,
  input: ListInput<'_>,
  (memory, latest): (&Memory, Option<MemoryId>),
) -> Element {
  let opened = state.opened.get();
  if let Some(mode) = opened_on(opened, memory) {
    let props = opened::editor(ctx, memory, mode, input.among());
    return ctx.mount_keyed::<RecordEditor>(&format!("memory-editor-{}-{}", memory.id(), memory.revision()), props);
  }
  if opened == Opened::Delete(memory.id()) {
    let props = opened::delete(ctx, memory, input.among());
    return ctx.mount_keyed::<DeleteConfirm>(&format!("memory-delete-{}", memory.id()), props);
  }
  if input.task_tab && memory.status() == MemoryStatus::Proposed {
    return proposal::proposal(ctx, (memory, input.records), input.origins, Place::Task);
  }
  let open = input.project.filter(|_| state.chosen() == Some(memory.id()));
  let props = RowProps {
    memory: memory.clone(),
    meta: words::meta(ctx, memory, input.origins, !input.task_tab),
    latest: Some(memory.id()) == latest,
    earlier: earlier_versions(ctx, memory, input),
    details: input
      .project
      .map(|_| if open.is_some() { Details::Open } else { Details::Closed }),
  };
  let row = ctx.mount_keyed::<RecordRow>(&format!("memory-row-{}", memory.id()), props);
  let note = row_note(ctx, state, memory.id()).map(note_line);
  let Some(project) = open else {
    let Some(note) = note else {
      return row;
    };
    return Column::new().width(Dimension::Pct(100.0)).child(row).child(note).into();
  };
  let details = ctx.mount_keyed::<RecordDetails>(
    &format!("memory-details-{}", memory.id()),
    DetailsProps {
      memory: Arc::clone(project),
      record: memory.id(),
    },
  );
  Column::new()
    .id(format!("memory-record-{}-open", memory.id()))
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(row)
    .with_children(note)
    .child(details)
    .into()
}

/// A row's note in its place under the row's text.
fn note_line(note: Row) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .right(SpacingSize::Md)
        .bottom(SpacingSize::Sm)
        .left(MemorySpacing::RowText.role()),
    )
    .child(note)
}

/// Why the last change of `record`, asked for from its row or its details, did nothing, under its row (`gQodn` in
/// `rKIl1`); `None` while something is open in a record's place, which says it there.
fn row_note(ctx: &Ctx, state: &MemoryState, record: MemoryId) -> Option<Row> {
  if state.opened.get() != Opened::Nothing {
    return None;
  }
  state.failure_note(ctx, &format!("memory-record-{record}"), Some(record))
}

/// The earlier versions of `memory`, newest first, as its history shows them.
fn earlier_versions(ctx: &Ctx, memory: &Memory, input: ListInput<'_>) -> Arc<[Earlier]> {
  listing::earlier_versions(memory, input.records)
    .into_iter()
    .map(|earlier| Earlier {
      text: earlier.content().text().into(),
      meta: words::meta(ctx, earlier, input.origins, !input.task_tab),
    })
    .collect()
}

/// *Show N earlier handoffs* with the steps they are of, or *Hide earlier handoffs* (`rXfGr`).
fn earlier_handoffs(ctx: &mut Ctx, state: &MemoryState, input: ListInput<'_>, earlier: &[&Memory]) -> Element {
  let open = state.earlier_handoffs.get();
  let label = match (open, earlier.len()) {
    (true, _) => ctx.t("memory.handoff.hideEarlier"),
    (false, 1) => ctx.t("memory.handoff.showEarlierOne"),
    (false, count) => ctx.t_args("memory.handoff.showEarlier", [("count", count.to_string())]),
  };
  // The newest first, as the design lists the steps they are of.
  let steps: Vec<String> = earlier
    .iter()
    .rev()
    .filter_map(|memory| input.origins.written(memory))
    .filter_map(|place| {
      let (number, name) = place.step?;
      let step = words::step(ctx, number, &name).to_string();
      Some(match place.attempt {
        Some(attempt) => ctx
          .t_args(
            "memory.handoff.stepAttempt",
            [("step", step), ("attempt", attempt.get().to_string())],
          )
          .to_string(),
        None => step,
      })
    })
    .collect();
  let toggle = state.earlier_handoffs.clone();
  let mut line = pressable(move || toggle.update(|open| *open = !*open))
    .id("memory-earlier-handoffs")
    .describe("a11y_name", label.clone())
    .padding(
      Padding::new()
        .left(MemorySpacing::TextIndent.role())
        .top(SpacingSize::Xs)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Xs)
    .child(icon(
      if open { Icon::ChevronDown } else { Icon::ChevronRight },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(Text::new(&label).variant(TypographyStyle::FieldLabel).nowrap());
  if !open && !steps.is_empty() {
    line = line.child(
      Text::new(&words::join(ctx, &steps))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    );
  }
  line.into()
}

/// *New record*'s editor, above the list while it is open.
pub fn new_record(ctx: &mut Ctx, state: &MemoryState) -> Option<Element> {
  if state.opened.get() != Opened::New {
    return None;
  }
  let props = opened::new_record(ctx, &state.project);
  Some(ctx.mount_keyed::<RecordEditor>("memory-editor-new", props))
}

/// Why a change of a record the list does not show did nothing (it is gone, or `filters` hide it now), above the
/// list while nothing is open in a record's place; a listed record says it under its row.
pub fn row_failure(ctx: &Ctx, state: &MemoryState, records: &[Memory], filters: &Filters) -> Option<Element> {
  if state.opened.get() != Opened::Nothing {
    return None;
  }
  let Some(Some(record)) = state.asked() else {
    return None;
  };
  if listing::lists(records, filters, record) {
    return None;
  }
  Some(state.failure_note(ctx, "memory-change", Some(record))?.into())
}
