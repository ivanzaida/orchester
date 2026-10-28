//! A record's details beside its page on the project's page (`Side` `Sajww` in 05c): who wrote it and where, its
//! history and the invocations that were given it, read off the UI thread when the page opens
//! ([`super::details_side`] lays them out).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::MemoryId,
  memory::{GivenMemory, Memory, MemoryStatus},
};
use orchester_workspace::{
  background,
  memory::{
    origin::Workplace,
    read::{GivenTo, ProjectMemory},
  },
  messages::{self, Message},
};

use super::{
  state::{MemoryState, Shared},
  words,
};
use crate::services::Services;

#[derive(Clone, PartialEq)]
pub struct DetailsProps {
  pub memory: Arc<ProjectMemory>,
  pub record: MemoryId,
}

impl DevtoolsInspectable for DetailsProps {}

pub struct RecordDetails {
  /// Every invocation that was given the record is listed.
  all_given: Signal<bool>,
}

impl Component for RecordDetails {
  type Props = DetailsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      all_given: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DetailsProps>().clone();
    let state = MemoryState::of(ctx);
    let Some(record) = props.memory.records.iter().find(|record| record.id() == props.record) else {
      return Element::new();
    };
    let given = given(ctx, &state, record);
    // A proposal under review has its own side (05e): it was given to no agent yet.
    if record.status() == MemoryStatus::Proposed {
      return super::proposal_side::side(ctx, &state, &props.memory, record);
    }
    super::details_side::side(ctx, &state, (&props.memory, record), (given, &self.all_given))
  }
}

/// The invocations given `record`, read when its page opens; `Err(None)` while they are read.
fn given(ctx: &mut Ctx, state: &MemoryState, record: &Memory) -> Result<Arc<GivenTo>, Option<Message>> {
  let hold = Services::of(ctx).hold;
  let read = ctx
    .future(
      (state.repo.clone(), record.id(), record.revision()),
      move |(repo, id, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || GivenTo::load_in(&repo, id).map(Shared::new))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get();
  if let Some(error) = read.error.clone().filter(|_| read.is_rejected()) {
    return Err(Some(error));
  }
  read.data.map(|given| given.0).ok_or(None)
}

/// Where an agent worked, without its role: *KON-8 · Run 104 · 02 Plan*.
pub(super) fn where_worked(ctx: &Ctx, place: Workplace) -> Arc<str> {
  let mut parts = Vec::new();
  parts.extend(place.task.as_ref().map(ToString::to_string));
  let place = Workplace {
    agent: None,
    task: None,
    ..place
  };
  parts.extend(words::workplace(ctx, &place, false));
  words::join(ctx, &parts)
}

/// Who an invocation was, why it was given the record, how many tokens and when (`j13iLJ`).
pub(super) fn invocation_why(ctx: &Ctx, invocation: &GivenMemory, place: &Workplace) -> Arc<str> {
  let mut why = Vec::new();
  why.extend(place.agent.as_deref().map(str::to_owned));
  why.push(reason(ctx, invocation.reason).to_string());
  why.push(
    ctx
      .t_args("memory.inspector.tokens", [("tokens", invocation.tokens.to_string())])
      .to_string(),
  );
  why.push(words::when(ctx, invocation.started_at));
  words::join(ctx, &why)
}

/// Why an invocation was given a record, as its details say it.
fn reason(ctx: &Ctx, reason: orchester_core::memory::InjectionReason) -> Arc<str> {
  use orchester_core::memory::InjectionReason;
  ctx.t(match reason {
    InjectionReason::Handoff => "memory.reason.handoff",
    InjectionReason::Pinned => "memory.reason.pinned",
    InjectionReason::Decision => "memory.reason.decision",
    InjectionReason::Warning => "memory.reason.warning",
    InjectionReason::Match => "memory.reason.match",
  })
}
