//! A proposal in the project page's queue (`Proposal` `f0kkti` in `05 Memory` `padSP`): a row of the queue's box with
//! its kind and meta line (*Decision · Project · proposed by Builder · Run 42 · confidence high*), what it says, the
//! record it would replace, then *Approve*, *Edit* (which opens *Review proposed record* in its place) and *Reject*
//! with *Agents don't receive it until you approve.* at the row's end.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::Memory;
use orchester_workspace::memory::{change::Change, origin::Origins};

use super::{
  parts, proposal,
  state::{MemoryState, Opened},
  words,
  work::Write,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// `memory`, a proposal, among `records` of the project and its tasks (which hold the record it would replace).
pub fn row(ctx: &mut Ctx, (memory, records): (&Memory, &[Memory]), origins: &Origins) -> Element {
  let state = MemoryState::of(ctx);
  let id = format!("memory-proposal-{}", memory.id());
  let meta = words::join(
    ctx,
    &[
      words::scope(ctx, memory, origins).to_string(),
      words::proposal_meta(ctx, memory, origins, true).to_string(),
    ],
  );
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&words::kind_name(ctx, memory.content().kind()))
        .id(format!("{id}-kind"))
        .variant(HubText::Kind.role())
        .nowrap(),
    )
    .child(
      Text::new(&meta)
        .id(format!("{id}-scope"))
        .variant(HubText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .with_children(memory.content().key().map(|key| {
      Text::new(key.as_str())
        .id(format!("{id}-key"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
    .with_children(parts::tags(&id, memory));
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(HubSpacing::Row.role())
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(
      Text::new(memory.content().text())
        .id(format!("{id}-text"))
        .variant(HubText::Statement.role()),
    )
    .with_children(proposal::supersedes(ctx, memory, records, origins, &id))
    .child(actions(ctx, &state, memory, &id))
    .into()
}

/// *Approve*, *Edit* and *Reject*, then the note that agents wait for the approval (`PNTFE`).
fn actions(ctx: &mut Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Element {
  let (record, revision) = (memory.id(), memory.revision());
  let busy = state.busy();
  let pending = state.pending_for(Some(record));
  let write = |change: Change| {
    let state = state.clone();
    move || state.write(Write::Change(record, revision, change.clone()))
  };
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let approve = ActionButton::new(
    format!("{id}-approve"),
    ButtonKind::Primary,
    ctx.t("memory.actions.approve"),
  )
  .icon(Icon::Check)
  .on_click(write(Change::Approve(None)));
  let approve = if pending { approve.loading() } else { held(approve) };
  let open = state.clone();
  let edit = held(
    ActionButton::new(format!("{id}-edit"), ButtonKind::Ghost, ctx.t("memory.actions.edit"))
      .icon(Icon::Pencil)
      .on_click(move || open.open(Opened::Review(record))),
  );
  let reject = held(
    ActionButton::new(
      format!("{id}-reject"),
      ButtonKind::DangerGhost,
      ctx.t("memory.actions.reject"),
    )
    .icon(Icon::X)
    .on_click(write(Change::Reject)),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(approve)
        .child(edit)
        .child(reject)
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&ctx.t("memory.proposal.note"))
            .variant(HubText::Hint.role())
            .nowrap(),
        ),
    )
    .with_children(state.failure_note(ctx, id, Some(record)))
    .into()
}
