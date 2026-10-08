//! The Conversation tab's timeline (`docs/design/orchester-workspace.md`, "Conversation (merged
//! with Activity)"): one run's transcript and conversation in the order they happened, step by step.
//! Each step execution that ran has its header, then in the order they happened its agent's
//! messages, the person's messages it took, its tool groups ([`group`]), the cards that wait for or
//! record a person's decision (question, plan, tool approval) and its problems on their own; its
//! result closes it. A tool request Orchester answered itself as it came (the profile's permissions, a
//! rule of the run, auto-approval) is no card: its call's entry in the tool group carries the mark. An attempt that
//! returned or restarted the work starts with its mark ([`mark`]). Messages that wait for a step follow, and a run that
//! ended closes with its end. What is shown of it is [`view`]'s.

pub mod group;
pub mod load;
pub mod mark;
pub mod question;
pub mod rail;
pub mod step;
pub mod view;

use std::sync::Arc;

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  integration::RunMerge,
  question::AskedQuestions,
  run::{DecidedBy, DecidedToolApproval, PendingToolApproval, Plan, RunStatus, StepOutcome, StepResult, StepStatus},
  time::Timestamp,
};

pub use self::{
  group::ToolGroup,
  mark::Mark,
  question::ChoiceCard,
  step::{StepHead, ToolRun},
};
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
  /// A return or restart, before the header of the step its attempt starts from.
  Mark(Mark),
  /// What an agent wrote (`V17jfc`); `step` is its step's name.
  AgentMessage {
    agent: Arc<str>,
    step: Arc<str>,
    text: Arc<str>,
  },
  /// A person's message, with its delivery.
  Message(MessageEntry),
  /// An agent's question, open while its step waits for the answer; `step` is its step's name, while
  /// the run's pipeline still has it.
  Question {
    entry: MessageEntry,
    step: Option<Arc<str>>,
  },
  Plan(PlanCard),
  Approval(ApprovalCard),
  /// An agent's question with suggested answers ([`question`]).
  Choice(ChoiceCard),
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
  /// What approving it starts, while it waits: the first step after it, if any.
  pub next: Option<NextStep>,
}

/// A step that starts once another passes: its name, and its agent unless a tool runs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextStep {
  pub step: Arc<str>,
  pub agent: Option<Arc<str>>,
}

/// A tool request a person decides (ORC-41): pending, decided or interrupted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalCard {
  pub request: PendingToolApproval,
  /// The step whose agent asked.
  pub step: Arc<str>,
  /// `None` while it waits; `Some(None)` when it was interrupted.
  pub decided: Option<Option<orchester_core::run::ToolApprovalDecision>>,
  /// Who answered it, once it is answered and that was recorded.
  pub decided_by: Option<orchester_core::run::DecidedBy>,
  /// What auto mode did with it, once it is answered; while it waits, the shown request says.
  pub review: Option<orchester_core::run::review::Review>,
}

/// How a step execution ended, as its result card says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultCard {
  pub execution: StepExecutionId,
  /// Its step's number in the pipeline.
  pub number: usize,
  pub step: Arc<str>,
  /// The agent role, or the tool's name for a tool step; `None` once that profile or tool is gone.
  pub agent: Option<Arc<str>>,
  pub passed: bool,
  pub tool_step: bool,
  /// The step judges the work: a failure returns the run to an earlier step (`AzKYQ`: *Review
  /// passed*).
  pub verdict: bool,
  /// What the agent's structured result said, when it wrote one.
  pub outcome: Option<StepOutcome>,
  /// What the agent or the tool reported; a plan's step reports nothing more, as its plan card
  /// holds the plan.
  pub reported: Option<Arc<str>>,
  pub took_millis: Option<i64>,
  pub tokens: u64,
  /// The files the step edited, each once.
  pub edited: usize,
  /// The step the run ended on or stopped at: its result is a card, another step's a divider (`uDqzB`,
  /// `kpnmP`).
  pub ended_on: bool,
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
  /// Its agents' questions with suggested answers.
  pub questions: &'a [AskedQuestions],
  pub merge: Option<&'a RunMerge>,
}

/// The run's timeline, step execution by step execution in the order they started.
pub fn build(sources: &Sources<'_>) -> Vec<Item> {
  let run = sources.run;
  let mut items = Vec::new();
  for (attempt, first, execution) in executions(run) {
    let Some(head) = StepHead::of(run, sources.entries, execution) else {
      continue;
    };
    let at = head.started.unwrap_or_else(|| run.run.created_at());
    if let Some(mark) = first.then(|| mark::mark_of(run, attempt)).flatten() {
      items.push(Item {
        key: format!("mark-{}", mark.attempt).into(),
        execution: Some(execution),
        at: mark.at.unwrap_or(at),
        kind: ItemKind::Mark(mark),
      });
    }
    items.push(Item {
      key: format!("step-{execution}").into(),
      execution: Some(execution),
      at,
      kind: ItemKind::Step(head),
    });
    items.extend(execution_items(sources, execution));
  }
  if *run.run.status() != RunStatus::Running {
    mark_ended_on(&mut items);
  }
  items.extend(StepHead::not_started(run).into_iter().map(|head| Item {
    key: format!("not-started-{}", head.number).into(),
    execution: None,
    at: run.run.created_at(),
    kind: ItemKind::Step(head),
  }));
  let answers = question::answer_messages(sources.questions);
  let mut waiting: Vec<Item> = sources
    .conversation
    .entries
    .iter()
    .filter(|entry| entry.execution.is_none() && !answers.contains(&entry.id))
    .map(|entry| message_item(entry, None))
    .collect();
  waiting.sort_by_key(|item| item.at);
  items.extend(waiting);
  items.extend(run_end(run, sources.merge));
  items
}

/// Marks the last step that ran, and its result: the step the run ended on or stopped at.
fn mark_ended_on(items: &mut [Item]) {
  let last = items.iter().rev().find_map(|item| match &item.kind {
    ItemKind::Step(head) => head.execution,
    _ => None,
  });
  let Some(last) = last else {
    return;
  };
  for item in items.iter_mut().filter(|item| item.execution == Some(last)) {
    match &mut item.kind {
      ItemKind::Step(head) => head.ended_on = true,
      ItemKind::Result(card) => card.ended_on = true,
      _ => {}
    }
  }
}

/// The run's step executions that ran, in the order they were created: the index of their attempt,
/// whether they are its first, and their id.
fn executions(run: &RunView) -> Vec<(usize, bool, StepExecutionId)> {
  run
    .run
    .attempts()
    .iter()
    .enumerate()
    .flat_map(|(index, attempt)| {
      attempt
        .executions()
        .iter()
        .enumerate()
        .map(move |(position, execution)| (index, position == 0, execution.id()))
    })
    .filter(|(.., execution)| run.invocations_of(*execution).next().is_some())
    .collect()
}

/// Everything of `execution` but its header: its transcript with the messages and cards in between,
/// in the order they happened, then its result.
fn execution_items(sources: &Sources<'_>, execution: StepExecutionId) -> Vec<Item> {
  let run = sources.run;
  let step = run.step_of(execution);
  let agent: Arc<str> = step.and_then(|step| step.agent.clone()).unwrap_or_else(|| "".into());
  let name: Arc<str> = step.map(|step| step.name.clone()).unwrap_or_else(|| "".into());
  let entries = &sources.conversation.entries;
  let (cards, mut hidden) = question::cards((sources.questions, entries), execution, (&agent, &name));
  hidden.extend(question::answer_messages(sources.questions));
  let mut between: Vec<Item> = entries
    .iter()
    .filter(|entry| entry.execution == Some(execution) && !hidden.contains(&entry.id))
    .filter(|entry| !matches!(entry.state, EntryState::Reply { .. }))
    .map(|entry| message_item(entry, step.map(|step| step.name.clone())))
    .collect();
  between.extend(cards);
  between.extend(
    run
      .plans
      .iter()
      .filter(|plan| plan.execution == execution)
      .map(|plan| plan_item(run, plan, (&agent, &name))),
  );
  between.extend(approval_items(sources, execution, &name));
  let mut items = interleaved(sources.entries, execution, (&agent, &name), between);
  items.extend(result_item(run, sources.entries, execution));
  items
}

/// The transcript of `execution` with `between` (its messages and cards) in time order: agent
/// messages, tool groups, problems, messages and cards. Anything that is not a tool event ends the
/// tool group before it, so a message sent between two tool calls splits their group. Of two at the
/// same time, the message or card comes first. A step result the agent wrote is its result card's,
/// not a message.
pub fn interleaved(
  entries: &[Entry],
  execution: StepExecutionId,
  (agent, step): (&Arc<str>, &Arc<str>),
  mut between: Vec<Item>,
) -> Vec<Item> {
  between.sort_by_key(|item| item.at);
  let mut between = between.into_iter().peekable();
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
  for entry in entries
    .iter()
    .filter(|entry| entry.execution == execution && !group::shown_as_card(entry))
  {
    while let Some(item) = between.next_if(|item| item.at <= entry.at) {
      flush(&mut tools, &mut items);
      items.push(item);
    }
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
  items.extend(between);
  items
}

/// A message of the step named `step`.
fn message_item(entry: &MessageEntry, step: Option<Arc<str>>) -> Item {
  let kind = match entry.state {
    EntryState::Question { .. } => ItemKind::Question {
      entry: entry.clone(),
      step,
    },
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
      next: waiting.then(|| next_step(run, plan)).flatten(),
    }),
  }
}

/// The first step of the run's pipeline that waits for the step of `plan`.
fn next_step(run: &RunView, plan: &Plan) -> Option<NextStep> {
  let key = &run
    .run
    .snapshot()
    .body()
    .steps()
    .iter()
    .find(|step| step.depends_on.contains(&plan.step))?
    .key;
  let step = run.steps.iter().find(|step| step.key == *key)?;
  Some(NextStep {
    step: step.name.clone(),
    agent: step.agent.clone().filter(|_| !step.tool_step),
  })
}

fn approval_items(sources: &Sources<'_>, execution: StepExecutionId, step: &Arc<str>) -> Vec<Item> {
  let pending = sources
    .pending
    .iter()
    .filter(|request| request.execution == execution)
    .map(|request| (request.clone(), None, None, None));
  let decided = sources
    .decided
    .iter()
    .filter(|decided| decided.request.execution == execution)
    .filter(|decided| decided.decided_by.is_none_or(DecidedBy::answered_waiting))
    .map(|decided| {
      (
        decided.request.clone(),
        Some(decided.decision),
        decided.decided_by,
        decided.review.clone(),
      )
    });
  pending
    .chain(decided)
    .map(|(request, decided, decided_by, review)| Item {
      key: approval_key(&request.id),
      execution: Some(execution),
      at: request.requested_at,
      kind: ItemKind::Approval(ApprovalCard {
        step: step.clone(),
        request,
        decided,
        decided_by,
        review,
      }),
    })
    .collect()
}

/// The key of the card of tool request `id`.
fn approval_key(id: &str) -> Arc<str> {
  format!("approval-{id}").into()
}

/// The key of the card of the tool request problem `entry` names: one that ended without a decision.
pub fn named_request(entry: &Entry) -> Option<Arc<str>> {
  match &entry.kind {
    EntryKind::Problem(RunnerProblem::DecisionInterrupted { request, .. }) => Some(approval_key(request)),
    _ => None,
  }
}

/// The key of the step whose items hold the item `key` among `items`; `None` when there is no such
/// item, or it belongs to no step.
pub fn step_of(items: &[Item], key: &str) -> Option<Arc<str>> {
  let execution = items.iter().find(|item| *item.key == *key)?.execution?;
  items
    .iter()
    .find(|item| matches!(item.kind, ItemKind::Step(_)) && item.execution == Some(execution))
    .map(|item| item.key.clone())
}

/// The result card of `execution` once it passed or failed, with what its transcript among `entries`
/// edited.
fn result_item(run: &RunView, entries: &[Entry], execution: StepExecutionId) -> Option<Item> {
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
  let planned = run.plans.iter().any(|plan| plan.execution == execution);
  let reported = run.reported(execution).filter(|_| !planned).map(|text| {
    let summary = StepResult::parse(text).map_or_else(|| text.to_owned(), |result| result.summary);
    // A tool step reports its command's end, then the output's last lines, which its command's row
    // already shows: the card keeps the first line (`hep0l`).
    let summary = if step.tool_step {
      summary.lines().next().unwrap_or_default()
    } else {
      summary.as_str()
    };
    Arc::<str>::from(summary)
  });
  Some(Item {
    key: format!("result-{execution}").into(),
    execution: Some(execution),
    at: ended.unwrap_or_else(|| run.run.created_at()),
    kind: ItemKind::Result(ResultCard {
      execution,
      number: step.number,
      step: step.name.clone(),
      agent: step.agent.clone(),
      passed,
      tool_step: step.tool_step,
      verdict: judges(run, &step.key),
      outcome: run
        .outputs
        .iter()
        .find(|output| output.execution == execution)
        .and_then(|output| output.outcome),
      reported: reported.filter(|text| !text.trim().is_empty()),
      took_millis: started
        .zip(ended)
        .map(|(started, ended)| ended.unix_millis() - started.unix_millis()),
      tokens: invocations
        .iter()
        .map(|invocation| invocation.usage)
        .sum::<orchester_core::run::TokenUsage>()
        .counted(),
      edited: edited(entries, execution),
      ended_on: false,
    }),
  })
}

/// The files `execution` edited among `entries`, each once.
fn edited(entries: &[Entry], execution: StepExecutionId) -> usize {
  entries
    .iter()
    .filter(|entry| entry.execution == execution)
    .filter_map(|entry| match &entry.kind {
      EntryKind::FileChange(change) => Some(&change.path),
      _ => None,
    })
    .collect::<std::collections::BTreeSet<_>>()
    .len()
}

/// Whether the step `key` judges the work: its failure returns the run to an earlier step.
fn judges(run: &RunView, key: &orchester_core::resource::StepKey) -> bool {
  run
    .run
    .snapshot()
    .body()
    .steps()
    .iter()
    .any(|step| step.key == *key && step.on_failure.is_some())
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
    ItemKind::Question { entry, .. } => entry.state == EntryState::Question { open: true },
    ItemKind::Plan(card) => card.waiting,
    ItemKind::Approval(card) => card.decided.is_none(),
    ItemKind::Choice(card) => card.waiting(),
    _ => false,
  }
}
