//! The Conversation tab's timeline (`docs/design/orchester-workspace.md`, "Conversation (merged
//! with Activity)"): one run's transcript and conversation in the order they happened, step by step.
//! Each step execution that ran has its header, then its agent's messages, the person's messages it
//! took, its tool groups ([`group`]), the cards that wait for or record a person's decision
//! (question, plan, tool approval), its problems on their own and its result; messages that wait for
//! a step follow, and a run that ended closes with its end. What is shown of it is [`view`]'s.

pub mod group;
pub mod load;
pub mod step;
pub mod view;

use std::sync::Arc;

use orchester_core::{
  id::StepExecutionId,
  integration::RunMerge,
  run::{DecidedToolApproval, PendingToolApproval, Plan, RunStatus, StepResult, StepStatus},
  time::Timestamp,
};

pub use self::{group::ToolGroup, step::StepHead};
use super::{
  conversation::{Conversation, Entry as MessageEntry, EntryState},
  detail::RunView,
  transcript::{Entry, EntryKind},
};

/// One item of the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
  /// Stable while the timeline grows.
  pub key: Arc<str>,
  /// The step execution it belongs to; `None` for a message that waits for one and the run's end.
  pub execution: Option<StepExecutionId>,
  pub at: Timestamp,
  pub kind: ItemKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemKind {
  Step(StepHead),
  /// What an agent wrote (`V17jfc`); `step` is its step's name.
  AgentMessage {
    agent: Arc<str>,
    step: Arc<str>,
    text: Arc<str>,
  },
  /// A person's message, with its delivery.
  Message(MessageEntry),
  /// An agent's question, open while its step waits for the answer.
  Question(MessageEntry),
  Plan(PlanCard),
  Approval(ApprovalCard),
  Tools(ToolGroup),
  /// A transcript entry that is never grouped: a denied command, an error, a runner problem.
  Problem(Entry),
  Result(ResultCard),
  /// The run ended.
  End(RunEnd),
}

/// A plan version of a gated step (`haIEI`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCard {
  pub plan: Plan,
  pub agent: Arc<str>,
  pub step: Arc<str>,
  /// The plan the run waits for a decision on.
  pub waiting: bool,
}

/// A tool request a person decides (ORC-41): pending, decided or interrupted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalCard {
  pub request: PendingToolApproval,
  /// `None` while it waits; `Some(None)` when it was interrupted.
  pub decided: Option<Option<orchester_core::run::ToolApprovalDecision>>,
}

/// How a step execution ended, as its result card says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultCard {
  pub execution: StepExecutionId,
  pub step: Arc<str>,
  pub passed: bool,
  pub tool_step: bool,
  /// What the agent or the tool reported.
  pub reported: Option<Arc<str>>,
  pub took_millis: Option<i64>,
  pub tokens: u64,
}

/// How the run ended (`AzKYQ`, `hep0l`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEnd {
  Ready,
  Merged(Arc<str>),
  Failed(Arc<str>),
  Cancelled,
}

/// What a run's timeline is made of besides its run.
pub struct Sources<'a> {
  pub run: &'a RunView,
  /// The run's transcript.
  pub entries: &'a [Entry],
  pub conversation: &'a Conversation,
  /// Its tool requests waiting for a decision, and the ones decided.
  pub pending: &'a [PendingToolApproval],
  pub decided: &'a [DecidedToolApproval],
  pub merge: Option<&'a RunMerge>,
}

/// The run's timeline, step execution by step execution in the order they started.
pub fn build(sources: &Sources<'_>) -> Vec<Item> {
  let run = sources.run;
  let mut items = Vec::new();
  for execution in executions(run) {
    let Some(head) = StepHead::of(run, execution) else {
      continue;
    };
    let at = head.started.unwrap_or_else(|| run.run.created_at());
    items.push(Item {
      key: format!("step-{execution}").into(),
      execution: Some(execution),
      at,
      kind: ItemKind::Step(head),
    });
    let mut own = execution_items(sources, execution);
    own.sort_by_key(|item| item.at);
    items.extend(own);
  }
  items.extend(StepHead::not_started(run).into_iter().map(|head| Item {
    key: format!("not-started-{}", head.number).into(),
    execution: None,
    at: run.run.created_at(),
    kind: ItemKind::Step(head),
  }));
  let mut waiting: Vec<Item> = sources
    .conversation
    .entries
    .iter()
    .filter(|entry| entry.execution.is_none())
    .map(message_item)
    .collect();
  waiting.sort_by_key(|item| item.at);
  items.extend(waiting);
  items.extend(run_end(run, sources.merge));
  items
}

/// The run's step executions that ran, in the order they were created.
fn executions(run: &RunView) -> Vec<StepExecutionId> {
  run
    .run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .map(|execution| execution.id())
    .filter(|execution| run.invocations_of(*execution).next().is_some())
    .collect()
}

/// Everything of `execution` but its header.
fn execution_items(sources: &Sources<'_>, execution: StepExecutionId) -> Vec<Item> {
  let run = sources.run;
  let step = run.step_of(execution);
  let agent: Arc<str> = step.and_then(|step| step.agent.clone()).unwrap_or_else(|| "".into());
  let name: Arc<str> = step.map(|step| step.name.clone()).unwrap_or_else(|| "".into());
  let mut items = transcript_items(sources.entries, execution, (&agent, &name));
  items.extend(
    sources
      .conversation
      .entries
      .iter()
      .filter(|entry| entry.execution == Some(execution))
      .filter(|entry| !matches!(entry.state, EntryState::Reply { .. }))
      .map(message_item),
  );
  items.extend(
    run
      .plans
      .iter()
      .filter(|plan| plan.execution == execution)
      .map(|plan| plan_item(run, plan, (&agent, &name))),
  );
  items.extend(approval_items(sources, execution));
  items.extend(result_item(run, execution));
  items
}

/// The transcript of `execution`: agent messages, tool groups and problems in order. A step result
/// the agent wrote is its result card's, not a message.
fn transcript_items(entries: &[Entry], execution: StepExecutionId, (agent, step): (&Arc<str>, &Arc<str>)) -> Vec<Item> {
  let mut items = Vec::new();
  let mut tools: Vec<Entry> = Vec::new();
  let flush = |tools: &mut Vec<Entry>, items: &mut Vec<Item>| {
    if let Some(first) = tools.first() {
      items.push(Item {
        key: format!("tools-{}-{}", first.sequence, first.part).into(),
        execution: Some(execution),
        at: first.at,
        kind: ItemKind::Tools(ToolGroup::new(std::mem::take(tools))),
      });
    }
  };
  for entry in entries.iter().filter(|entry| entry.execution == execution) {
    if group::tool_kind(entry).is_some() {
      tools.push(entry.clone());
      continue;
    }
    let kind = match &entry.kind {
      EntryKind::Message(text) if StepResult::parse(text).is_some() => continue,
      EntryKind::Message(text) => ItemKind::AgentMessage {
        agent: agent.clone(),
        step: step.clone(),
        text: text.clone(),
      },
      _ => ItemKind::Problem(entry.clone()),
    };
    flush(&mut tools, &mut items);
    items.push(Item {
      key: format!("entry-{}-{}", entry.sequence, entry.part).into(),
      execution: Some(execution),
      at: entry.at,
      kind,
    });
  }
  flush(&mut tools, &mut items);
  items
}

fn message_item(entry: &MessageEntry) -> Item {
  let kind = match entry.state {
    EntryState::Question { .. } => ItemKind::Question(entry.clone()),
    _ => ItemKind::Message(entry.clone()),
  };
  Item {
    key: format!("message-{}", entry.id).into(),
    execution: entry.execution,
    at: entry.at,
    kind,
  }
}

fn plan_item(run: &RunView, plan: &Plan, (agent, step): (&Arc<str>, &Arc<str>)) -> Item {
  let waiting = plan.review.is_none()
    && run.run.status() == &RunStatus::Running
    && run.run.awaiting_plan().contains(&plan.execution)
    && Plan::latest_of(&run.plans, plan.execution).is_some_and(|latest| latest.version == plan.version);
  Item {
    key: format!("plan-{}-{}", plan.execution, plan.version).into(),
    execution: Some(plan.execution),
    at: plan.created_at,
    kind: ItemKind::Plan(PlanCard {
      plan: plan.clone(),
      agent: agent.clone(),
      step: step.clone(),
      waiting,
    }),
  }
}

fn approval_items(sources: &Sources<'_>, execution: StepExecutionId) -> Vec<Item> {
  let pending = sources
    .pending
    .iter()
    .filter(|request| request.execution == execution)
    .map(|request| (request.clone(), None));
  let decided = sources
    .decided
    .iter()
    .filter(|decided| decided.request.execution == execution)
    .map(|decided| (decided.request.clone(), Some(decided.decision)));
  pending
    .chain(decided)
    .map(|(request, decided)| Item {
      key: format!("approval-{}", request.id).into(),
      execution: Some(execution),
      at: request.requested_at,
      kind: ItemKind::Approval(ApprovalCard { request, decided }),
    })
    .collect()
}

/// The result card of `execution` once it passed or failed.
fn result_item(run: &RunView, execution: StepExecutionId) -> Option<Item> {
  let status = run.run.execution(execution)?.status();
  let passed = match status {
    StepStatus::Passed => true,
    StepStatus::Failed => false,
    _ => return None,
  };
  let step = run.step_of(execution)?;
  let invocations: Vec<_> = run.invocations_of(execution).collect();
  let started = invocations.first().map(|invocation| invocation.started_at);
  let ended = invocations.iter().filter_map(|invocation| invocation.ended_at).max();
  let reported = run.reported(execution).map(|text| {
    let summary = StepResult::parse(text).map_or_else(|| text.to_owned(), |result| result.summary);
    Arc::<str>::from(summary.as_str())
  });
  Some(Item {
    key: format!("result-{execution}").into(),
    execution: Some(execution),
    at: ended.unwrap_or_else(|| run.run.created_at()),
    kind: ItemKind::Result(ResultCard {
      execution,
      step: step.name.clone(),
      passed,
      tool_step: step.tool_step,
      reported: reported.filter(|text| !text.trim().is_empty()),
      took_millis: started
        .zip(ended)
        .map(|(started, ended)| ended.unix_millis() - started.unix_millis()),
      tokens: invocations
        .iter()
        .map(|invocation| invocation.usage)
        .sum::<orchester_core::run::TokenUsage>()
        .counted(),
    }),
  })
}

fn run_end(run: &RunView, merge: Option<&RunMerge>) -> Option<Item> {
  let end = match (run.run.status(), merge) {
    (RunStatus::Succeeded, Some(merge)) => RunEnd::Merged(merge.into.as_str().into()),
    (RunStatus::Succeeded, None) => RunEnd::Ready,
    (RunStatus::Failed(_), _) => RunEnd::Failed(
      run
        .steps
        .iter()
        .find(|step| step.status == Some(StepStatus::Failed))
        .map_or_else(|| "".into(), |step| step.name.clone()),
    ),
    (RunStatus::Cancelled, _) => RunEnd::Cancelled,
    _ => return None,
  };
  Some(Item {
    key: "end".into(),
    execution: None,
    at: run.finished_at.unwrap_or_else(|| run.run.created_at()),
    kind: ItemKind::End(end),
  })
}

/// Whether `item` waits for the person: an open question, a plan that waits, a pending request.
pub fn waits_for_you(item: &Item) -> bool {
  match &item.kind {
    ItemKind::Question(entry) => entry.state == EntryState::Question { open: true },
    ItemKind::Plan(card) => card.waiting,
    ItemKind::Approval(card) => card.decided.is_none(),
    _ => false,
  }
}
