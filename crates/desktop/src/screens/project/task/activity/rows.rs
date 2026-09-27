//! The rows of the Activity transcript: one header per step execution (`mDnc3`) followed by its
//! entries (`N4SlQq`) while the execution is open. Finished executions start collapsed and the
//! running one open; failed commands start expanded and others collapsed. A person's choice
//! overrides the start state and is kept while the transcript grows.

use std::{collections::HashMap, sync::Arc};

use lurq::core::Signal;
use orchester_core::{
  connection::AgentProvider,
  id::StepExecutionId,
  run::{StepStatus, TokenUsage},
  time::Timestamp,
};

use crate::tasks::{
  detail::RunView,
  transcript::{Entry, EntryKind},
};

/// Rows a person opened or closed, by row key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct OpenRows(HashMap<Arc<str>, bool>);

impl lurq::app::component::DevtoolsInspectable for OpenRows {}

/// Open or closed per row key, where a person changed the start state.
#[derive(Clone)]
pub(super) struct Toggles(pub(super) Signal<OpenRows>);

impl Toggles {
  pub(super) fn is_open(&self, key: &str, default: bool) -> bool {
    self.0.get().0.get(key).copied().unwrap_or(default)
  }

  pub(super) fn flip(&self, key: &Arc<str>, default: bool) {
    let key = key.clone();
    self.0.update(move |rows| {
      let open = rows.0.get(&key).copied().unwrap_or(default);
      rows.0.insert(key, !open);
    });
  }
}

/// A step execution's header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Header {
  pub(super) execution: StepExecutionId,
  pub(super) number: usize,
  pub(super) step: Arc<str>,
  /// The agent role, or the tool's name for a tool step.
  pub(super) agent: Arc<str>,
  /// A tool step: a command tool ran instead of an agent, on no model and with no tokens.
  pub(super) command: bool,
  pub(super) provider: Option<AgentProvider>,
  pub(super) model: Arc<str>,
  pub(super) invocations: usize,
  pub(super) started: Option<Timestamp>,
  /// `None` while an invocation runs.
  pub(super) ended: Option<Timestamp>,
  pub(super) usage: TokenUsage,
  pub(super) status: Option<StepStatus>,
  pub(super) open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RowKind {
  Header(Header),
  Entry { entry: Entry, agent: Arc<str>, open: bool },
}

/// One row of the virtualized transcript.
#[derive(Clone)]
pub(super) struct TranscriptRow {
  pub(super) key: Arc<str>,
  pub(super) kind: RowKind,
  /// Whether unparsed lines are shown line by line.
  pub(super) raw: bool,
  pub(super) toggles: Toggles,
}

impl PartialEq for TranscriptRow {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && self.kind == other.kind && self.raw == other.raw
  }
}

impl lurq::app::component::DevtoolsInspectable for TranscriptRow {}

/// Rows of the executions of `run` (only `filter`'s when given) with their entries.
pub(super) fn rows(
  run: &RunView,
  entries: &[Entry],
  filter: Option<StepExecutionId>,
  raw: bool,
  toggles: &Toggles,
) -> Vec<TranscriptRow> {
  let mut rows = Vec::new();
  let executions = run
    .run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .filter(|execution| filter.is_none_or(|wanted| wanted == execution.id()))
    .filter(|execution| run.invocations_of(execution.id()).next().is_some());
  for execution in executions {
    let Some(header) = header(run, execution.id(), execution.status(), toggles) else {
      continue;
    };
    let open = header.open;
    let agent = header.agent.clone();
    rows.push(row(
      format!("header-{}", execution.id()),
      RowKind::Header(header),
      raw,
      toggles,
    ));
    if !open {
      continue;
    }
    for entry in entries.iter().filter(|entry| entry.execution == execution.id()) {
      let key: Arc<str> = format!("entry-{}", entry.sequence).into();
      let open = toggles.is_open(&key, entry_starts_open(entry));
      let kind = RowKind::Entry {
        entry: entry.clone(),
        agent: agent.clone(),
        open,
      };
      rows.push(TranscriptRow {
        key,
        kind,
        raw,
        toggles: toggles.clone(),
      });
    }
  }
  rows
}

fn row(key: String, kind: RowKind, raw: bool, toggles: &Toggles) -> TranscriptRow {
  TranscriptRow {
    key: key.into(),
    kind,
    raw,
    toggles: toggles.clone(),
  }
}

fn header(run: &RunView, execution: StepExecutionId, status: StepStatus, toggles: &Toggles) -> Option<Header> {
  let step = run.step_of(execution)?;
  let invocations: Vec<_> = run.invocations_of(execution).collect();
  let running = invocations.iter().any(|invocation| invocation.ended_at.is_none());
  let key = format!("header-{execution}");
  Some(Header {
    execution,
    number: step.number,
    step: step.name.clone(),
    agent: step.agent.clone().unwrap_or_else(|| "".into()),
    command: step.tool_step,
    provider: step.provider,
    model: step.model.clone().unwrap_or_else(|| "".into()),
    invocations: invocations.len(),
    started: invocations.first().map(|invocation| invocation.started_at),
    ended: if running {
      None
    } else {
      invocations.last().and_then(|invocation| invocation.ended_at)
    },
    usage: invocations.iter().map(|invocation| invocation.usage).sum(),
    status: Some(status),
    open: toggles.is_open(&key, running || status.is_open()),
  })
}

/// Failures start open; commands, reasoning and unparsed lines start closed.
fn entry_starts_open(entry: &Entry) -> bool {
  match entry.kind {
    EntryKind::Command(_) | EntryKind::ToolCommand(_) | EntryKind::Reasoning(_) | EntryKind::Raw(_) => {
      entry.is_failure()
    }
    _ => true,
  }
}
