//! The panel's pinned actions (`Actions` `iPRRp` in Y1, `KAhLt` in Y2, `xMSTT` in Y6), by the record's status: a
//! proposal's *Approve*, *Edit* (its review in the panel) and *Reject*, with that agents don't receive it until it is
//! approved; an active record's *Edit*, *Deprecate* and *Pin* or *Unpin*; a deprecated one's *Restore*, which makes it
//! active again as it was, with no question as it loses nothing; and at the end of every record's bar *Delete…*, which
//! asks in a modal. Superseded and rejected records have *Delete…* only. While a change is under way the others wait,
//! and the one pressed shows that it works.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::memory::{Memory, MemoryStatus};
use orchester_workspace::memory::change::Change;

use crate::{
  screens::{
    memory::{
      state::{MemoryState, Opened},
      work::Write,
    },
    team_hub::bar::bar_ending,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub(super) fn bar(ctx: &Ctx, state: &MemoryState, record: &Memory) -> Element {
  let (id, revision) = (record.id(), record.revision());
  let busy = state.busy();
  let pending = state.pending_for(Some(id));
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let write = |change: Change| {
    let state = state.clone();
    move || state.write(Write::Change(id, revision, change.clone()))
  };
  let open = |opened: Opened| {
    let state = state.clone();
    move || state.open(opened)
  };
  let button = |name: &str, kind: ButtonKind, label: &str, glyph: Icon| {
    ActionButton::new(format!("memory-panel-{name}"), kind, ctx.t(label))
      .icon(glyph)
      .pinned()
  };
  let mut buttons = Vec::new();
  let mut note = Some("memory.editor.saveNote");
  match record.status() {
    MemoryStatus::Proposed => {
      let approve = button("approve", ButtonKind::Primary, "memory.actions.approve", Icon::Check)
        .on_click(write(Change::Approve(None)));
      buttons.push(if pending { approve.loading() } else { held(approve) });
      buttons.push(held(
        button("edit", ButtonKind::Secondary, "memory.actions.edit", Icon::Pencil).on_click(open(Opened::Review(id))),
      ));
      buttons.push(held(
        button("reject", ButtonKind::DangerGhost, "memory.actions.reject", Icon::X).on_click(write(Change::Reject)),
      ));
      note = Some("memory.proposal.note");
    }
    MemoryStatus::Active => {
      buttons.push(held(
        button("edit", ButtonKind::Secondary, "memory.actions.edit", Icon::Pencil).on_click(open(Opened::Edit(id))),
      ));
      buttons.push(held(
        button(
          "deprecate",
          ButtonKind::Secondary,
          "memory.actions.deprecate",
          Icon::Archive,
        )
        .on_click(write(Change::Deprecate)),
      ));
      let pinned = record.is_pinned();
      let (label, glyph) = if pinned {
        ("memory.actions.unpin", Icon::PinOff)
      } else {
        ("memory.actions.pin", Icon::Pin)
      };
      buttons.push(held(
        button("pin", ButtonKind::Ghost, label, glyph).on_click(write(Change::Pin(!pinned))),
      ));
    }
    MemoryStatus::Deprecated => {
      let restore = button(
        "restore",
        ButtonKind::Primary,
        "memory.actions.restore",
        Icon::ArchiveRestore,
      )
      .on_click(write(Change::Restore));
      buttons.push(if pending { restore.loading() } else { held(restore) });
    }
    MemoryStatus::Superseded | MemoryStatus::Rejected | MemoryStatus::Withdrawn => note = None,
  }
  let delete = (record.status() != MemoryStatus::Proposed).then(|| {
    held(
      button("delete", ButtonKind::DangerGhost, "memory.editor.delete", Icon::Trash2)
        .on_click(open(Opened::Delete(id))),
    )
    .into()
  });
  bar_ending(
    "memory-panel-actions",
    buttons.into_iter().map(Element::from).collect(),
    note.map(|key| (ctx.t(key).to_string(), lurq::app::theme::PaletteColor::TextMuted)),
    delete,
  )
}
