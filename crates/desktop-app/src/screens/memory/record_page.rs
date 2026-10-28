//! A record's own page on the project's Memory tab (`05c Memory record` `CF0R7`, `05e Review proposal` `Ybk74`): the
//! crumb back to the tab, the record's tile, title and what it is with its actions at the right — *Deprecate* and
//! *Delete…* on an active record, *Restore* and *Delete…* on a deprecated one, *Delete…* on the others — then *Delete
//! this record?* while it is asked, and the editor (an active record, a proposal under review) or what the record
//! says; beside them who wrote it, its history and the invocations that were given it. A press on a record's row, its
//! *Edit* or *Delete*, or a proposal's *Edit* opens it; the crumb and *Cancel* go back to the list.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::{Memory, MemoryStatus};
use orchester_workspace::memory::{change::Change, origin::Origins, read::ProjectMemory};

use super::{
  delete::DeleteConfirm,
  details::{DetailsProps, RecordDetails},
  editor::{EditorMode, EditorProps, RecordEditor},
  opened::{self, Among},
  parts as memory_parts,
  state::{MemoryState, Opened},
  tabs, words,
  work::Write,
};
use crate::{
  routes::MemoryTab,
  screens::hub::{
    self, page_head,
    parts::{self, Entry},
  },
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The page's main column (`Pane` `s3C7x5`) and its side (`Side` `Sajww`).
const MAIN_WIDTH: f32 = 680.0;
const SIDE_WIDTH: f32 = 424.0;

/// The page of the record the project's page opened, on `tab`; `None` while none is open, or it is gone.
pub fn page(ctx: &mut Ctx, state: &MemoryState, memory: &Arc<ProjectMemory>, tab: MemoryTab) -> Option<Element> {
  if state.task.is_some() {
    return None;
  }
  let id = state.page_record()?;
  let record = memory
    .waiting
    .iter()
    .chain(&memory.records)
    .find(|record| record.id() == id)?
    .clone();
  let mut scope: Vec<Memory> = memory.records.clone();
  scope.extend(memory.replaced.iter().cloned());
  let among = Among {
    records: &scope,
    origins: &memory.origins,
    with_task: true,
    project: Some(memory),
  };
  let mode = match record.status() {
    MemoryStatus::Proposed => Some(EditorMode::Review),
    MemoryStatus::Active => Some(EditorMode::Edit),
    MemoryStatus::Deprecated | MemoryStatus::Superseded | MemoryStatus::Rejected | MemoryStatus::Withdrawn => None,
  };
  let editor = mode.map(|mode| opened::editor(ctx, &record, mode, among));
  let mut main = Column::new()
    .id("memory-record-page")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Sections.role())
    .child(top(ctx, state, (&record, editor.as_ref()), (tab, &memory.origins)));
  if state.opened.get() == Opened::Delete(id) {
    let props = opened::delete(ctx, &record, among);
    main = main.child(ctx.mount_keyed::<DeleteConfirm>(&format!("memory-delete-{id}"), props));
  }
  main = main.child(match editor.clone() {
    Some(props) => ctx.mount_keyed::<RecordEditor>(&format!("memory-editor-{}-{}", id, record.revision()), props),
    None => said(ctx, &record),
  });
  let side = if memory.records.iter().any(|kept| kept.id() == id) {
    ctx.mount_keyed::<RecordDetails>(
      &format!("memory-details-{id}"),
      DetailsProps {
        memory: Arc::clone(memory),
        record: id,
      },
    )
  } else {
    proposal_side(ctx, state, editor.as_ref())
  };
  Some(hub::page_body(
    "memory-record",
    (main.into(), MAIN_WIDTH),
    (side, SIDE_WIDTH),
  ))
}

/// The crumb and the page's head with the record's actions (`hErL9`); why a change failed under them while no editor
/// says it.
fn top(
  ctx: &mut Ctx,
  state: &MemoryState,
  (record, editor): (&Memory, Option<&EditorProps>),
  (tab, origins): (MemoryTab, &Origins),
) -> Element {
  let back = {
    let state = state.clone();
    Arc::new(move |_| state.leave_page()) as crate::navigation::link::Open
  };
  let list = ctx.t("memory.page.crumb");
  let here = tabs::name(ctx, tab);
  let crumb = page_head::crumb("memory-record-crumb", (&list, back), Some(&here));
  let kind = record.content().kind();
  let review = record.status() == MemoryStatus::Proposed;
  let title = if review {
    ctx.t_args("memory.editor.reviewTitle", [("kind", &*words::kind_lower(ctx, kind))])
  } else {
    match record.content().key() {
      Some(key) => ctx.t_args(
        "memory.page.titleKeyed",
        [
          ("kind", words::kind_name(ctx, kind).to_string()),
          ("key", key.as_str().to_owned()),
        ],
      ),
      None => words::kind_name(ctx, kind),
    }
  };
  let sub = match editor {
    Some(props) if review => props.subtitle.clone(),
    _ => ctx.t_args(
      "memory.page.sub",
      [
        ("status", status_word(ctx, record.status()).to_string()),
        ("meta", words::meta(ctx, record, origins, true).to_string()),
      ],
    ),
  };
  let head = page_head::head(ctx, "memory-record", (words::glyph(kind), &title, &sub), None);
  let failure = editor
    .is_none()
    .then(|| state.failure_note(ctx, "memory-record-page", Some(record.id())))
    .flatten();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role())
    .child(crumb)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(lurq::app::theme::SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(head))
        .with_children(actions(ctx, state, record)),
    )
    .with_children(failure)
    .into()
}

/// The record's status as a word: *Active*.
fn status_word(ctx: &Ctx, status: MemoryStatus) -> Arc<str> {
  ctx.t(match status {
    MemoryStatus::Proposed => "memory.proposal.waiting",
    MemoryStatus::Active => "memory.status.active",
    MemoryStatus::Deprecated => "memory.status.deprecated",
    MemoryStatus::Superseded => "memory.status.superseded",
    MemoryStatus::Rejected => "memory.status.rejected",
    MemoryStatus::Withdrawn => "memory.status.withdrawn",
  })
}

/// *Deprecate* and *Delete…*, *Restore* and *Delete…*, or *Delete…* (`BfMjB`, `gjFED`); none on a proposal.
fn actions(ctx: &Ctx, state: &MemoryState, record: &Memory) -> Vec<Element> {
  let (id, revision) = (record.id(), record.revision());
  let busy = state.busy();
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let write = |change: Change| {
    let state = state.clone();
    move || state.write(Write::Change(id, revision, change.clone()))
  };
  let mut actions: Vec<ActionButton> = Vec::new();
  match record.status() {
    MemoryStatus::Proposed => return Vec::new(),
    MemoryStatus::Active => actions.push(held(
      ActionButton::new(
        "memory-page-deprecate",
        ButtonKind::Ghost,
        ctx.t("memory.actions.deprecate"),
      )
      .icon(Icon::Archive)
      .on_click(write(Change::Deprecate)),
    )),
    MemoryStatus::Deprecated => actions.push(held(
      ActionButton::new(
        "memory-page-restore",
        ButtonKind::Secondary,
        ctx.t("memory.actions.restore"),
      )
      .icon(Icon::RotateCcw)
      .on_click(write(Change::Restore)),
    )),
    MemoryStatus::Superseded | MemoryStatus::Rejected | MemoryStatus::Withdrawn => {}
  }
  let delete = state.clone();
  actions.push(held(
    ActionButton::new(
      "memory-page-delete",
      ButtonKind::DangerGhost,
      ctx.t("memory.editor.delete"),
    )
    .icon(Icon::Trash2)
    .on_click(move || delete.open(Opened::Delete(id))),
  ));
  actions.into_iter().map(Element::from).collect()
}

/// What a record no one edits any more says, as the *Record* group's rows (`ymE2V`).
fn said(ctx: &Ctx, record: &Memory) -> Element {
  let content = record.content();
  let value = |id: &str, label: &str, value: &str| -> Element {
    parts::field(
      id,
      label,
      None,
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(HubText::Value.role()),
    )
    .into()
  };
  let mut rows = vec![
    value(
      "memory-record-type",
      &ctx.t("memory.editor.type"),
      &words::kind_name(ctx, content.kind()),
    ),
    value(
      "memory-record-importance",
      &ctx.t("memory.editor.importance"),
      &words::importance(ctx, content.importance()),
    ),
  ];
  if let Some(key) = content.key() {
    rows.insert(1, value("memory-record-key", &ctx.t("memory.editor.key"), key.as_str()));
  }
  rows.push(
    parts::stacked_field(
      "memory-record-text",
      (&ctx.t("memory.editor.text"), None),
      Text::new(content.text())
        .id("memory-record-text-value")
        .variant(HubText::Value.role())
        .selectable(true),
      None,
    )
    .into(),
  );
  if let Some(tags) = memory_parts::tags("memory-record", record) {
    rows.push(parts::stacked_field("memory-record-tags", (&ctx.t("memory.editor.tags"), None), tags, None).into());
  }
  let id = "memory-record-group";
  parts::group(
    id,
    parts::head(id, &ctx.t("memory.editor.record"), None, None),
    parts::boxed(&format!("{id}-rows"), rows),
  )
  .into()
}

/// Beside a proposal under review: where it came from and the agent's confidence (`H2OVV` in 05e).
fn proposal_side(ctx: &mut Ctx, state: &MemoryState, editor: Option<&EditorProps>) -> Element {
  let Some(props) = editor else {
    return Element::new();
  };
  let mut rows: Vec<Element> = Vec::new();
  for (index, (label, value)) in props.facts.iter().enumerate() {
    let mut row = Entry::new(format!("memory-review-fact-{index}"), label.clone()).hint(value.clone());
    if index == 0
      && let Some(task) = &props.source_task
    {
      let words = ctx.t_args("memory.editor.sourceLinkTask", [("task", task.to_string())]);
      row = row.end(memory_parts::conversation_link(
        ctx,
        (state.project_id, task),
        ("memory-review-source-link", &words),
      ));
    }
    rows.push(row.into());
  }
  let id = "memory-review-source";
  parts::group(
    id,
    parts::head(id, &ctx.t("memory.editor.source"), None, None),
    parts::boxed(&format!("{id}-rows"), rows),
  )
  .into()
}
