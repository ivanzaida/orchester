//! The lists the selection bar opens (`Bulk move menu` in `Bulk Move to refuses those cards` `Y27GJ`): the columns by
//! status, each taking the selected cards that can go there and saying which stay and why, or unavailable with why
//! when none can; the priorities; and the project's tags, a tag every selected card has taken off them all. A move
//! asks first where a single card's drop asks: tasks whose runs ended moved to Todo (Don't move, Move to Todo or Run
//! again), queuing drafts that start at once, and marking as done work that waits to be merged.

use lurq::app::ctx::Ctx;
use orchester_core::task::organization::{Priority, WorkflowStatus};
use orchester_workspace::tasks::{
  close::Closing,
  moves::{self, Decision, Move},
};

use super::{
  actions::{Ask, Change, ChangeKind},
  ask::listed,
  bulk::close_selected,
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

/// What a move of the selected cards to a column comes to: the changes it makes and the drafts among them it queues,
/// the tasks whose runs ended that it runs again or only moves (each with its Run again change), the cards it closes
/// and those of them whose work waits to be merged, and the cards that stay with why.
#[derive(Default)]
pub struct BulkMove {
  pub moving: Vec<Change>,
  pub queued: Vec<Subject>,
  pub ended: Vec<(Subject, Change)>,
  pub closing: Vec<Subject>,
  pub unmerged: Vec<Subject>,
  pub staying: Vec<(Subject, moves::Refusal)>,
}

/// What moving `cards` to `status` does to each, by the rules a single card's drop follows.
pub fn bulk_move(dropping: &Dropping, cards: &[CardModel], status: WorkflowStatus) -> BulkMove {
  let mut found = BulkMove::default();
  for card in cards {
    let subject = dropping.subject(card);
    match moves::decide(&card.row, status) {
      Decision::Stay => {}
      Decision::Refused(refusal) => found.staying.push((subject, refusal)),
      Decision::Move(Move::Close(_)) => {
        if moves::leaves_unmerged(&card.row) {
          found.unmerged.push(subject.clone());
        }
        found.closing.push(subject);
      }
      Decision::Move(Move::Queue) => {
        found.queued.push(subject);
        found.moving.push(change(card, ChangeKind::Queue));
      }
      Decision::Move(Move::ReturnToDraft) => found.moving.push(change(card, ChangeKind::ReturnToDraft)),
      Decision::Move(Move::Status(status)) => found.moving.push(change(card, ChangeKind::Status(status))),
      // A latest run that cannot be read is refused, as a single card's drop refuses it.
      Decision::Move(Move::RunAgain) => match card.latest_run {
        Some(after) => {
          let again = ChangeKind::RunAgain {
            after,
            revision: card.revision,
            pipeline: card.pipeline.clone(),
          };
          found.ended.push((subject, change(card, again)));
        }
        None => found.staying.push((subject, moves::Refusal::HasRun)),
      },
    }
  }
  found
}

/// Makes the changes of a move, asking first, as a single card's drop on Todo does, when it takes tasks whose runs
/// ended (they run again or only move), and when it queues drafts that start at once: agent slots are free and new
/// starts are not held. Both asked, the drafts are asked about after the answer about the tasks whose runs ended.
fn change_or_ask(dropping: &Dropping, (moving, queued, ended): (Vec<Change>, Vec<Subject>, Vec<(Subject, Change)>)) {
  let starts = dropping.free.filter(|free| *free > 0 && !queued.is_empty());
  if ended.is_empty() {
    match starts {
      Some(free) => dropping.state.asked.set(Some(Ask::BulkQueue {
        queued,
        free,
        changes: moving,
      })),
      None => dropping.state.change(moving),
    }
    return;
  }
  // Move to Todo starts nothing: the drafts that would start wait for their own question.
  let (queues, changes) = match starts {
    Some(_) => moving.into_iter().partition(|change| change.kind == ChangeKind::Queue),
    None => (Vec::new(), moving),
  };
  let (ended, again) = ended.into_iter().unzip();
  dropping.state.asked.set(Some(Ask::BulkRunAgain {
    ended,
    again,
    changes,
    then: starts.map(|free| {
      Box::new(Ask::BulkQueue {
        queued,
        free,
        changes: queues,
      })
    }),
  }));
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
        queued,
        ended,
        closing,
        unmerged,
        staying,
      } = bulk_move(dropping, cards, status);
      if moving.is_empty() && ended.is_empty() && closing.is_empty() {
        let reason = staying.first().map_or_else(
          || ctx.t("tasks.board.bulk.allThere"),
          |(_, refusal)| ctx.t(reason_key(*refusal)),
        );
        return MenuItem::unavailable(id, label, reason).icon(glyph).without_lock();
      }
      let dropping = dropping.clone();
      let item = MenuItem::new(id, label, move || {
        if !closing.is_empty() {
          let closing_as = if status == WorkflowStatus::Done {
            Closing::Done
          } else {
            Closing::Canceled
          };
          close_selected(&dropping.state, closing_as, (closing.clone(), unmerged.clone()));
        }
        change_or_ask(&dropping, (moving.clone(), queued.clone(), ended.clone()));
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
      let changes: Vec<Change> = cards
        .iter()
        .map(|card| change(card, ChangeKind::Priority(priority)))
        .collect();
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
