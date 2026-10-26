//! The Orchestrator's answer to a turn (`Answer` `nfXCZ` in `01 Conversation` `wRSL4`): a 24 avatar with `waypoints`,
//! *Orchestrator* and when it began, and under them its parts 10 apart: its replies as every chat draws what an agent
//! wrote (`ui::chat::agent_text`), the actions it took as event lines, several in a row as one bordered list
//! (`Actions` `J0N1u`), what waits for the person as decision cards (`Proposal` `FDetP`), a hold of new starts, and a
//! new session's divider. Under a wait of an agent, the same parts nest as a quote, without the avatar
//! (`ANSWERED BY THE ORCHESTRATOR` in `GIsJh`).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  orchestrator::{ActionOutcome, ChatAction, ChatEvent, HeldWork, SessionChange},
  time::Timestamp,
};
use orchester_workspace::display;

use super::{
  change::{self, ChangeCard, ChangeProps},
  ending::divider,
  held::{HeldCard, HeldProps},
  line::{list, nested as quoted},
  proposal::{ProposalProps, StartProposal},
  timeline::TurnRow,
  turns::Part,
};
use crate::{
  theme::ExtraSpacing,
  ui::chat::{Avatar, agent_text, chat_row},
};

/// The answer under `row`'s opening, with the Orchestrator's avatar and name; `None` before its first part.
pub(super) fn answer(ctx: &mut Ctx, row: &TurnRow) -> Option<Element> {
  let first = row.turn.answer.first()?;
  let time = display::clock(first.at());
  let key = row.turn.key();
  Some(chat_row(
    format!("orchestrator-answer-{key}"),
    Avatar::Orchestrator,
    (&ctx.t("orchestrator.title"), &time),
    vec![parts(ctx, row).into()],
  ))
}

/// The answer of the turn a wait started by itself, nested under the wait's line as a quote; `None` before its first
/// part.
pub(super) fn nested(ctx: &mut Ctx, row: &TurnRow) -> Option<Element> {
  row.turn.answer.first()?;
  let key = row.turn.key();
  Some(quoted(&format!("orchestrator-answer-{key}"), parts(ctx, row)))
}

/// The answer's parts, 10 apart; event lines in a row are one list, whose lines say no time.
fn parts(ctx: &mut Ctx, row: &TurnRow) -> Column {
  let key = row.turn.key();
  let mut shown: Vec<Element> = Vec::new();
  let mut lines: Vec<Element> = Vec::new();
  let flush = |shown: &mut Vec<Element>, lines: &mut Vec<Element>, index: usize| match lines.len() {
    0 => {}
    1 => shown.append(lines),
    _ => shown.push(list(&format!("orchestrator-actions-{key}-{index}"), std::mem::take(lines))),
  };
  let answer = &row.turn.answer;
  let line_at = |index: usize| {
    answer
      .get(index)
      .is_some_and(|part| matches!(part, Part::Action { action, .. } if is_line(row, action)))
  };
  for (index, part) in answer.iter().enumerate() {
    let id = format!("orchestrator-action-{key}-{index}");
    match part {
      Part::Action { action, at, seq } if is_line(row, action) => {
        let listed = index.checked_sub(1).is_some_and(line_at) || line_at(index + 1);
        lines.push(action_element(ctx, row, &id, (action, *at, *seq), listed));
      }
      Part::Action { action, at, seq } => {
        flush(&mut shown, &mut lines, index);
        shown.push(action_element(ctx, row, &id, (action, *at, *seq), false));
      }
      Part::Reply { text: said, .. } => {
        flush(&mut shown, &mut lines, index);
        shown.push(agent_text(
          ctx,
          &format!("orchestrator-reply-{key}-{index}"),
          said,
          (row.env.project, &row.env.names),
          &row.env.links,
        ));
      }
      Part::Event {
        event: ChatEvent::NewSession { because },
        ..
      } => {
        flush(&mut shown, &mut lines, index);
        let words = match because {
          SessionChange::Connection => ctx.t("orchestrator.newSession.connection"),
          SessionChange::Account => ctx.t("orchestrator.newSession.account"),
        };
        shown.push(divider(&format!("orchestrator-event-{key}-{index}"), &words, None));
      }
    }
  }
  flush(&mut shown, &mut lines, row.turn.answer.len());
  Column::new()
    .id(format!("orchestrator-answer-{key}-parts"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(shown)
}

/// Whether `action` shows as an event line, which joins the lines around it in one list: everything but what waits
/// for the person, a proposed start and a hold of new starts.
fn is_line(row: &TurnRow, action: &ChatAction) -> bool {
  if action.change.is_some() || action.auto_mode {
    return !change::waits(action);
  }
  match &action.outcome {
    ActionOutcome::Proposed(_) => row.env.scope.is_none(),
    ActionOutcome::Done => action.held.is_none() || row.env.scope.is_none(),
    _ => true,
  }
}

/// The line or card of `action`, taken at `at` as entry `seq` of the conversation, its line without its time when
/// `listed` in the answer's list; `id` names it.
fn action_element(
  ctx: &mut Ctx,
  row: &TurnRow,
  id: &str,
  (action, at, seq): (&ChatAction, Timestamp, i64),
  listed: bool,
) -> Element {
  let id = id.to_owned();
  if action.change.is_some() || action.auto_mode {
    return ctx.mount_keyed::<ChangeCard>(
      &id,
      ChangeProps {
        id: id.clone(),
        scope: row.env.scope.clone(),
        project: (row.env.project, Arc::from(row.env.name.as_ref())),
        seq,
        action: action.clone(),
        at,
        listed,
      },
    );
  }
  match (&action.outcome, &row.env.scope) {
    (ActionOutcome::Proposed(start), Some(scope)) => ctx.mount_keyed::<StartProposal>(
      &id,
      ProposalProps {
        id: id.clone(),
        scope: scope.clone(),
        seq,
        task: action.task.clone().unwrap_or_default(),
        start: (**start).clone(),
        applying: action.claimed.is_some(),
        at,
      },
    ),
    (ActionOutcome::Done, Some(scope)) if action.held.is_some() => ctx.mount_keyed::<HeldCard>(
      &id,
      HeldProps {
        id: id.clone(),
        project: (row.env.project, row.env.name.to_string()),
        root: scope.root.clone(),
        task: action.task.clone(),
        held: action.held.clone().unwrap_or(HeldWork {
          group: None,
          waiting: Vec::new(),
        }),
        at,
      },
    ),
    _ => super::action::line(ctx, &id, (action, at, listed), row.env.project),
  }
}
