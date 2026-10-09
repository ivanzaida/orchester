//! The cards that wait for or record a person's decision (`Cards and rows inside the conversation` `YhiGC`):
//! a question, a plan, a tool request, a question with suggested answers. They are `screens::task::cards`',
//! shown here at the chat's width.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::tasks::timeline::Item;

use super::{Timeline, message::links, step_label};
use crate::screens::task::cards::{self, Place};

/// The card of `item`, a question, a plan, a tool request or a question with suggested answers.
pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, item: &Item) -> Element {
  let place = Place {
    project: timeline.project,
    detail: timeline.detail.clone(),
    key: item.key.clone(),
    step: timeline.step_of(ctx, item.execution),
    step_label,
    links: links(),
  };
  cards::card(ctx, place, item)
}
