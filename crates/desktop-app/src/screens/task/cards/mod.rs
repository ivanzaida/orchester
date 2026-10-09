//! The cards that wait for or record a person's decision in a task's conversation (`Cards and rows inside the
//! conversation` `YhiGC`; `Question card` `tKYgO`, `Plan card` `Q5J0r`, `Approval card` `gwOaJ` in `Task page
//! · components` `n70u8y`): an agent's question ([`question`]), its question with suggested answers
//! ([`choice`]), a plan of a gated step ([`plan`]) and a tool request ([`approval`]). A waiting card carries the
//! `warning` stroke; a decided one goes quiet and says who decided, and when. Each card answers through the
//! project's runner, as the person ([`action`]); there is no dialog (D6). Free text, a question's answer or the
//! changes a plan needs, is the composer's ([`Decisions`]).
//!
//! While the run lost its runner (it is interrupted until one looks after it), no card waits or can be
//! answered: each says so.
//!
//! A card that already waited when the tab opened takes the focus, so the status bar's *Review* and a
//! Needs-you card that open the conversation land on it; the chat scrolls to it (`timeline::follow`).

mod action;
mod approval;
mod choice;
mod decisions;
mod frame;
mod plan;
mod question;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Column, node::Element};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  runner::state::RunnerState,
  tasks::{
    detail::TaskDetail,
    timeline::{Item, ItemKind},
  },
};

pub use self::decisions::Decisions;
use crate::{project::Projects, ui::markdown::LinkHandler};

/// Where a card is shown.
#[derive(Clone)]
pub struct Place {
  pub project: ProjectId,
  /// The task, with the run the card belongs to.
  pub detail: Arc<TaskDetail>,
  /// "03 Implement", the step the asking agent works in, while the run's pipeline has it.
  pub step: Option<Arc<str>>,
  /// What a press on a link in what an agent wrote does.
  pub links: LinkHandler,
}

impl PartialEq for Place {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.detail == other.detail
      && self.step == other.step
      && Arc::ptr_eq(&self.links, &other.links)
  }
}

impl Place {
  /// The run is recorded as running while no runner looks after it (interrupted until one does): no agent
  /// waits for an answer now, and none can be given.
  fn lost(&self, ctx: &mut Ctx) -> bool {
    let online = Projects::of(ctx)
      .feed(self.project)
      .status
      .get()
      .is_some_and(|status| *status.state() == RunnerState::Online);
    self.detail.lost_its_runner(online)
  }

  /// The run's number in the project; 0 while the task has no readable run.
  fn run_number(&self) -> u32 {
    self.detail.run.as_ref().map_or(0, |run| run.run.number().get())
  }
}

/// The card of `item` at `place`: a question, a question with suggested answers, a plan or a tool request;
/// nothing for an item that is none of these.
pub fn card(ctx: &mut Ctx, place: Place, item: &Item) -> Element {
  match &item.kind {
    ItemKind::Question { entry, .. } => question::card(ctx, place, entry),
    ItemKind::Choice(card) => choice::card(ctx, place, card),
    ItemKind::Plan(card) => plan::card(ctx, place, card),
    ItemKind::Approval(card) => approval::card(ctx, place, card),
    _ => Column::new().into(),
  }
}
