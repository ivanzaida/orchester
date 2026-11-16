//! What a team task's state card says of who closed it or blocked it, and when (`Task - v2` `jDN0r` T4 `V26A3`, T5
//! `eJKwy`, T6 `Xwa1j`, T11a–f `ujJ6U`, `Tydl9`, `x0xP1`, `dTn0v`, `b3t84e`, `vyQJx`), from the server's records of
//! them. A task whose record the server did not send (an older server) keeps the plain state word, with no person and
//! no time: missing is unknown, never "nobody".

use lurq::app::ctx::Ctx;
use orchester_core::task::organization::WorkflowStatus;

use crate::{
  screens::settings::orchester::moment,
  team::pages::{TeamAttribution, TeamClosed, TeamClosedHow, TeamPerson},
};

/// The title and the detail of a Done or Canceled task's card.
pub struct ClosedWords {
  pub title: String,
  pub detail: String,
}

/// `status` (Done or Canceled) as its card says it, given the record of its closing.
pub fn closed(ctx: &Ctx, status: WorkflowStatus, record: Option<&TeamClosed>) -> ClosedWords {
  let done = status == WorkflowStatus::Done;
  let plain = |title: &str, detail: &str| ClosedWords {
    title: ctx.t(title).to_string(),
    detail: ctx.t(detail).to_string(),
  };
  let (plain_title, plain_detail) = if done {
    ("summary.team.lifecycle.doneTitle", "summary.team.lifecycle.doneDetail")
  } else {
    (
      "summary.team.lifecycle.canceledTitle",
      "summary.team.lifecycle.canceledDetail",
    )
  };
  let Some(record) = record else {
    return plain(plain_title, plain_detail);
  };
  let time = moment(ctx, record.at);
  match &record.how {
    TeamClosedHow::Person(by) => {
      let (named, you) = if done {
        ("summary.team.lifecycle.doneBy", "summary.team.lifecycle.doneByYou")
      } else {
        (
          "summary.team.lifecycle.canceledBy",
          "summary.team.lifecycle.canceledByYou",
        )
      };
      ClosedWords {
        title: person_at(ctx, (named, you), by, &time),
        detail: ctx.t(plain_detail).to_string(),
      }
    }
    TeamClosedHow::Delivery { module_name } => ClosedWords {
      title: ctx
        .t_args(
          "summary.team.lifecycle.doneDelivery",
          [("module", module_name.as_str())],
        )
        .to_string(),
      detail: ctx
        .t_args("summary.team.lifecycle.doneDeliveryDetail", [("time", time.as_str())])
        .to_string(),
    },
    TeamClosedHow::Run => ClosedWords {
      title: ctx.t("summary.team.lifecycle.doneRun").to_string(),
      detail: ctx
        .t_args("summary.team.lifecycle.doneRunDetail", [("time", time.as_str())])
        .to_string(),
    },
  }
}

/// A blocked task's title: who set its reason and when, or the plain word when the server does not say.
pub fn blocked_title(ctx: &Ctx, record: Option<&TeamAttribution>) -> String {
  match record {
    Some(record) => person_at(
      ctx,
      (
        "summary.team.lifecycle.blockedBy",
        "summary.team.lifecycle.blockedByYou",
      ),
      &record.by,
      &moment(ctx, record.at),
    ),
    None => ctx.t("summary.team.lifecycle.blockedTitle").to_string(),
  }
}

/// `by` at `time` in the words `(named, you)`: the second when `by` is the person signed in.
fn person_at(ctx: &Ctx, (named, you): (&str, &str), by: &TeamPerson, time: &str) -> String {
  if by.you {
    ctx.t_args(you, [("time", time)]).to_string()
  } else {
    ctx
      .t_args(named, [("person", by.name.as_str()), ("time", time)])
      .to_string()
  }
}
