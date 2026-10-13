//! The cards that wait for or record a person's decision in a task's conversation (`Cards and rows inside the
//! conversation` `YhiGC`; `Question card` `tKYgO`, `Plan card` `Q5J0r`, `Approval card` `gwOaJ` in `Task page
//! · components` `n70u8y`): an agent's question ([`question`]), its question with suggested answers
//! ([`choice`]), a plan of a gated step ([`plan`]) and a tool request ([`approval`]). A waiting card carries the
//! `warning` stroke; a decided one goes quiet and says who decided, and when. Each card answers through the
//! project's runner, as the person; there is no dialog (D6). What a card was given and what the person picked
//! on it are the tab's ([`Decisions`]), so they outlive the card in the chat's list. Free text, a question's
//! answer or the changes a plan needs, is the composer's.
//!
//! While the run lost its runner (it is interrupted until one looks after it), no card waits or can be
//! answered: each says so.
//!
//! The card that waits for the person when the chat first shows its rows, which the chat reveals, takes the
//! focus, so the status bar's *Review* and a Needs-you card that open the conversation land on it.

mod approval;
mod choice;
mod decisions;
mod frame;
mod plan;
mod question;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Column, node::Element};
use orchester_core::id::{ProjectId, RunId};
use orchester_workspace::{
  tasks::{
    detail::TaskDetail,
    timeline::{Item, ItemKind},
  },
  workspace::OpenProject,
};

pub use self::decisions::Decisions;
use super::conversation::ComposerState;
use crate::{project::Projects, ui::markdown::LinkHandler};

/// How a step is named in the conversation: "03 Implement" for step 3, Implement.
pub type StepLabel = fn(&Ctx, usize, &str) -> Arc<str>;

/// Where a card is shown.
#[derive(Clone)]
pub struct Place {
  pub project: ProjectId,
  /// The task, with the run the card belongs to.
  pub detail: Arc<TaskDetail>,
  /// The card's key in the timeline.
  pub key: Arc<str>,
  /// "03 Implement", the step the asking agent works in, while the run's pipeline has it.
  pub step: Option<Arc<str>>,
  pub step_label: StepLabel,
  /// What a press on a link in what an agent wrote does.
  pub links: LinkHandler,
  /// Shown in the title bar's Needs you panel, whose item frames a waiting card and says what it is: the card
  /// leaves out its own frame and head there (`YDJsC` in `zwuZt`).
  pub in_panel: bool,
}

impl PartialEq for Place {
  /// The step label's function is the conversation's own, the same for every card.
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.detail == other.detail
      && self.key == other.key
      && self.step == other.step
      && self.in_panel == other.in_panel
      && Arc::ptr_eq(&self.links, &other.links)
  }
}

impl Place {
  /// The run the card belongs to.
  fn run(&self) -> RunId {
    self
      .detail
      .run
      .as_ref()
      .map(|run| run.id())
      .expect("a card is an item of its task's run's timeline")
  }

  /// The run's number in the project; 0 while the task has no readable run.
  fn run_number(&self) -> u32 {
    self.detail.run.as_ref().map_or(0, |run| run.run.number().get())
  }

  /// The open project the card's answers go to the runner of; `None` while it is not open.
  fn open(&self, ctx: &mut Ctx) -> Option<OpenProject> {
    Projects::of(ctx)
      .state(self.project)
      .and_then(|state| state.open().cloned())
  }

  /// The composer under the chat, where a question's answer and a plan's changes are written; `None` where a
  /// card is shown without one.
  fn composer(ctx: &mut Ctx) -> Option<ComposerState> {
    ctx.use_context::<ComposerState>()
  }

  /// "03 Implement" for the step of the card's run named `step`, while its pipeline has it; else its name.
  fn label_of(&self, ctx: &Ctx, step: &str) -> Arc<str> {
    let number = self
      .detail
      .run
      .as_ref()
      .and_then(|run| run.steps.iter().find(|view| &*view.name == step))
      .map(|view| view.number);
    match number {
      Some(number) => (self.step_label)(ctx, number, step),
      None => step.into(),
    }
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
