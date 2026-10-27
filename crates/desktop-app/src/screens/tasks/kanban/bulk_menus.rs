//! The lists the selection bar opens (`Bulk move menu` in `Bulk Move to refuses those cards` `Y27GJ`): the columns by
//! status, each taking the selected cards that can go there and saying which stay and why, or unavailable with why
//! when none can; the priorities; and the project's tags, a tag every selected card has taken off them all.

use lurq::app::ctx::Ctx;
use orchester_core::task::organization::{Priority, WorkflowStatus};
use orchester_workspace::tasks::{
  close::Closing,
  moves::{self, Decision, Move},
};

use super::{
  actions::{Change, ChangeKind},
  ask::listed,
  bulk::Bulk,
  card::CardModel,
  drop::Dropping,
  refusal::reason_key,
};
use crate::{
  screens::task_close::Subject,
  ui::{
    icon::Icon,
    menu::MenuItem,
    status::{priority_look, workflow_key, workflow_look},
  },
};

/// What a move of the selected cards to a column comes to: the changes it makes, the cards it closes, and the cards
/// that stay with why.
#[derive(Default)]
pub struct BulkMove {
  pub moving: Vec<Change>,
  pub closing: Vec<Subject>,
  pub staying: Vec<(Subject, moves::Refusal)>,
}

/// What moving `cards` to `status` does to each, by the rules a single card's drop follows; Run again is asked one
/// task at a time, from its own card.
pub fn bulk_move(dropping: &Dropping, cards: &[CardModel], status: WorkflowStatus) -> BulkMove {
  let mut found = BulkMove::default();
  for card in cards {
    let subject = dropping.subject(card);
    match moves::decide(&card.row, status) {
      Decision::Stay => {}
      Decision::Refused(refusal) => found.staying.push((subject, refusal)),
      Decision::Move(Move::Close(_)) => found.closing.push(subject),
      Decision::Move(Move::Queue) => found.moving.push(change(card, ChangeKind::Queue)),
      Decision::Move(Move::ReturnToDraft) => found.moving.push(change(card, ChangeKind::ReturnToDraft)),
      Decision::Move(Move::Status(status)) => found.moving.push(change(card, ChangeKind::Status(status))),
      Decision::Move(Move::RunAgain) => found.staying.push((subject, moves::Refusal::HasRun)),
    }
  }
  found
}

/// The columns by status.
pub fn move_rows(ctx: &Ctx, dropping: &Dropping, cards: &[CardModel]) -> Vec<MenuItem> {
  WorkflowStatus::ALL
    .into_iter()
    .map(|status| {
      let id = format!("tasks-board-bulk-move-{}", super::model::status_name(status));
      let (label, glyph) = (ctx.t(workflow_key(status)), workflow_look(status).0);
      let BulkMove {
        moving,
        closing,
        staying,
      } = bulk_move(dropping, cards, status);
      if moving.is_empty() && closing.is_empty() {
        let reason = staying
          .first()
          .map_or_else(|| ctx.t("tasks.board.bulk.allThere"), |(_, refusal)| ctx.t(reason_key(*refusal)));
        return MenuItem::unavailable(id, label, reason).icon(glyph).without_lock();
      }
      let state = dropping.state.clone();
      let item = MenuItem::new(id, label, move || {
        if !closing.is_empty() {
          let closing_as = if status == WorkflowStatus::Done {
            Closing::Done
          } else {
            Closing::Canceled
          };
          state.bulk.run(Bulk::Prepare(closing_as, closing.clone()));
        }
        state.change(moving.clone());
      })
      .icon(glyph);
      match staying.first() {
        Some((_, refusal)) => item.detail(ctx.t_args(
          "tasks.board.bulk.stay",
          [
            ("tasks", listed(ctx, staying.iter().map(|(task, _)| task))),
            ("reason", ctx.t(reason_key(*refusal)).to_string()),
          ],
        )),
        None => item,
      }
    })
    .collect()
}

pub fn priority_rows(ctx: &Ctx, dropping: &Dropping, cards: &[CardModel]) -> Vec<MenuItem> {
  Priority::ALL
    .into_iter()
    .map(|priority| {
      let (glyph, key, _) = priority_look(priority);
      let changes: Vec<Change> = cards.iter().map(|card| change(card, ChangeKind::Priority(priority))).collect();
      let state = dropping.state.clone();
      MenuItem::new(
        format!("tasks-board-bulk-priority-{}", super::model::priority_name(priority)),
        ctx.t(key),
        move || state.change(changes.clone()),
      )
      .icon(glyph)
    })
    .collect()
}

/// The project's tags: a tag every selected card has is taken off them all, any other added to those without it.
pub fn tag_rows(ctx: &Ctx, dropping: &Dropping, cards: &[CardModel]) -> Vec<MenuItem> {
  if dropping.tags.is_empty() {
    return vec![
      MenuItem::unavailable(
        "tasks-board-bulk-tags-none",
        ctx.t("tasks.board.menu.noTags"),
        ctx.t("tasks.board.menu.noTagsReason"),
      )
      .without_lock(),
    ];
  }
  dropping
    .tags
    .iter()
    .enumerate()
    .map(|(index, (tag, name))| {
      let all = cards.iter().all(|card| card.row.organization.tags().contains(tag));
      let changes: Vec<Change> = cards
        .iter()
        .map(|card| {
          let mut tags = card.row.organization.tags().to_vec();
          tags.retain(|held| held != tag);
          if !all {
            tags.push(*tag);
          }
          change(card, ChangeKind::Tags(tags))
        })
        .collect();
      let state = dropping.state.clone();
      MenuItem::new(format!("tasks-board-bulk-tag-{index}"), name.clone(), move || {
        state.change(changes.clone())
      })
      .icon(Icon::Tag)
      .chosen(all)
    })
    .collect()
}

fn change(card: &CardModel, kind: ChangeKind) -> Change {
  Change {
    task: card.row.id,
    key: card.row.key.clone(),
    kind,
  }
}
