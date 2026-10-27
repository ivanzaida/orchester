//! A record waiting for approval on a task's Memory tab (`Memory / Proposal row` `LdHs2`): on `surface_raised`, what
//! it says with its meta line and tags, the record it would replace (*Supersedes 1 active record · key by author*),
//! *Waits for your approval*, and *Reject*, *Edit* (which opens *Review proposed record* in its place) and *Approve*
//! after the note that agents do not receive it until then. The project page's queue lists its proposals as rows
//! instead ([`super::queue_row`]).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::Memory;
use orchester_workspace::memory::{change::Change, origin::Origins};

use super::{
  parts,
  state::{MemoryState, Opened},
  words,
  work::Write,
};
use crate::{
  theme::{ExtraRadius, ExtraTypography, MemorySpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// Where a proposal is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listed {
  /// The task's Memory tab: one task's proposals.
  Task,
  /// The project's page: every scope's proposals in one queue.
  Queue,
}

/// `memory`, a proposal, among `records` of its scope (which hold the record it would replace).
pub fn proposal(ctx: &mut Ctx, (memory, records): (&Memory, &[Memory]), origins: &Origins, listed: Listed) -> Element {
  if listed == Listed::Queue {
    return super::queue_row::row(ctx, (memory, records), origins);
  }
  let state = MemoryState::of(ctx);
  let id = format!("memory-proposal-{}", memory.id());
  let head = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(memory.content().key().map(|key| {
      Text::new(key.as_str())
        .id(format!("{id}-key"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }));
  let body = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(head)
    .child(
      Text::new(memory.content().text())
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&words::proposal_meta(ctx, memory, origins, false)).variant(TypographyStyle::Caption))
        .with_children(parts::tags(&id, memory)),
    )
    .with_children(supersedes(ctx, memory, records, origins, &id));
  // `Waiting` (`V6r5PT`) sits at the row's top right, beside the body, so the text stops short of it.
  let waiting = parts::status(
    &format!("{id}-waiting"),
    (Icon::Hand, ctx.t("memory.proposal.waiting")),
    PaletteColor::Warning,
  );
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      crate::theme::ExtraSpacing::Inset.role(),
      SpacingSize::Md,
    ))
    .spacing(crate::theme::ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(parts::glyph(words::glyph(memory.content().kind())))
        .child(body)
        .child(waiting),
    )
    .child(actions(ctx, &state, memory, &id))
    .into()
}

/// *Supersedes 1 active record · key by author*, when the proposal names an active record it would replace.
pub(super) fn supersedes(
  ctx: &Ctx,
  memory: &Memory,
  records: &[Memory],
  origins: &Origins,
  id: &str,
) -> Option<Element> {
  let replaced = records.iter().find(|record| {
    Some(record.id()) == memory.supersedes() && record.status() == orchester_core::memory::MemoryStatus::Active
  })?;
  let author = origins
    .written(replaced)
    .and_then(|place| place.agent)
    .unwrap_or_else(|| ctx.t("memory.meta.youLower"));
  let label = match replaced.content().key() {
    Some(key) => ctx.t_args(
      "memory.proposal.supersedesKeyed",
      [("key", key.as_str()), ("author", &*author)],
    ),
    None => ctx.t_args("memory.proposal.supersedes", [("author", &*author)]),
  };
  Some(
    parts::status(
      &format!("{id}-supersedes"),
      (Icon::Replace, label),
      PaletteColor::TextSecondary,
    )
    .into(),
  )
}

/// *Agents don't receive it until you approve.*, then *Reject*, *Edit* and *Approve* (`foPMI`).
fn actions(ctx: &mut Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Element {
  let (record, revision) = (memory.id(), memory.revision());
  let busy = state.busy();
  let pending = state.pending_for(Some(record));
  let write = |change: Change| {
    let state = state.clone();
    move || state.write(Write::Change(record, revision, change.clone()))
  };
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let reject = held(
    ActionButton::new(
      format!("{id}-reject"),
      ButtonKind::Ghost,
      ctx.t("memory.actions.reject"),
    )
    .icon(Icon::X)
    .on_click(write(Change::Reject)),
  );
  let open = state.clone();
  let edit = held(
    ActionButton::new(
      format!("{id}-edit"),
      ButtonKind::Secondary,
      ctx.t("memory.actions.edit"),
    )
    .icon(Icon::Pencil)
    .on_click(move || open.open(Opened::Review(record))),
  );
  let approve = ActionButton::new(
    format!("{id}-approve"),
    ButtonKind::Primary,
    ctx.t("memory.actions.approve"),
  )
  .icon(Icon::Check)
  .on_click(write(Change::Approve(None)));
  let approve = if pending { approve.loading() } else { held(approve) };
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(MemorySpacing::TextIndent.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("memory.proposal.note")).variant(TypographyStyle::Caption));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      row
        .child(Spacer::new().flex(1.0))
        .child(reject)
        .child(edit)
        .child(approve),
    )
    .with_children(state.failure_note(ctx, id, Some(record)))
    .into()
}
