//! Permission changes shared by the pipeline editor and domain MCP tools.

use orchester_core::resource::{Pipeline, Step, StepKey, StepKind, Weaker, weaker_than};

use super::data::{Assignables, OpenedPipeline};
use crate::messages::Message;

/// What publishing permits beyond the previous version; the first version needs a person.
pub fn publish_reasons(latest: Option<&Pipeline>, new: &Pipeline) -> Vec<Message> {
  let step_name = |key: &StepKey| {
    new
      .step(key)
      .map_or_else(|| key.as_str().to_owned(), |step| step.name.as_str().to_owned())
  };
  match latest {
    None => vec![Message::new("appMcp.widens.firstPipelineVersion")],
    Some(latest) => weaker_than(latest, new)
      .into_iter()
      .map(|weaker| match weaker {
        Weaker::Steps => Message::new("appMcp.widens.pipelineSteps"),
        Weaker::Sources => Message::new("appMcp.widens.pipelineSources"),
        Weaker::Step(key) => Message::new("appMcp.widens.pipelineStep").with("step", step_name(&key)),
      })
      .collect(),
  }
}

/// What saving drops from the draft or published version, or newly permits a tool to run.
pub fn draft_reasons(current: &OpenedPipeline, body: &Pipeline) -> Vec<Message> {
  // Neither the published version's approvals nor those a person added to the draft since are dropped.
  let mut dropped = weakened_gates(current.body.steps(), body.steps());
  if let Some(latest) = &current.latest {
    for reason in weakened_gates(latest.body.steps(), body.steps()) {
      if !dropped.contains(&reason) {
        dropped.push(reason);
      }
    }
  }
  let latest_steps = current.latest.as_ref().map_or(&[][..], |latest| latest.body.steps());
  dropped.extend(added_tool_steps(
    [current.body.steps(), latest_steps],
    body.steps(),
    &current.assignables,
  ));

  dropped
}

/// The approvals `steps` drop from `base`, the latest published version (or, before the first, the draft as
/// it stands): a step that waits for a person's approval, or asks a person about its tool calls, and no
/// longer does or is gone. Dropping one is a person's decision; other narrowing within the profile is not.
fn weakened_gates(base: &[Step], steps: &[Step]) -> Vec<Message> {
  let asks =
    |step: &Step| matches!(&step.kind, StepKind::Agent { permissions, .. } if permissions.interactive_tool_approvals);
  let mut reasons = Vec::new();
  for old in base {
    let new = steps.iter().find(|step| step.key == old.key);
    let named = |key: &'static str| Message::new(key).with("step", old.name.as_str().to_owned());
    if old.approval_gate && !new.is_some_and(|step| step.approval_gate) {
      reasons.push(named("appMcp.widens.approvalGate"));
    }
    if asks(old) && !new.is_some_and(asks) {
      reasons.push(named("appMcp.widens.stepToolCalls"));
    }
  }
  reasons
}

/// The steps of `steps` that call a tool, or run the test suites, where neither the draft as it stands nor the
/// latest published version (`bases`) does under the same key: each runs programs on this computer, which a person
/// decides.
fn added_tool_steps(bases: [&[Step]; 2], steps: &[Step], assignables: &Assignables) -> Vec<Message> {
  let runs = |step: &Step| match step.kind {
    StepKind::Tool { tool } => tool.map(Runs::Tool),
    StepKind::Test => Some(Runs::Tests),
    StepKind::Agent { .. } => None,
  };
  steps
    .iter()
    .filter_map(|step| Some((step, runs(step)?)))
    .filter(|(step, what)| {
      !bases
        .iter()
        .flat_map(|base| base.iter())
        .any(|before| before.key == step.key && runs(before) == Some(*what))
    })
    .map(|(step, what)| match what {
      Runs::Tool(tool) => {
        let name = assignables
          .tool(tool)
          .map_or_else(|| tool.to_string(), |choice| choice.name.to_string());
        Message::new("appMcp.projectTools.widens.toolStep")
          .with("step", step.name.as_str().to_owned())
          .with("tool", name)
      }
      Runs::Tests => Message::new("appMcp.projectTools.widens.testStep").with("step", step.name.as_str().to_owned()),
    })
    .collect()
}

/// What a step runs on this computer besides an agent.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Runs {
  Tool(orchester_core::id::ResourceId),
  Tests,
}
