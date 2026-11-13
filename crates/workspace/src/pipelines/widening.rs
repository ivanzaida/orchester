//! Permission changes shared by the pipeline editor and domain MCP tools.

use orchester_core::resource::{Pipeline, Step, StepKey, StepKind, Weaker, weaker_than};

use super::data::Assignables;
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
        Weaker::StopsWaiting { step, waited_for } => Message::new("appMcp.widens.pipelineStopsWaiting")
          .with("step", step_name(&step))
          .with(
            "waited",
            waited_for.iter().map(&step_name).collect::<Vec<_>>().join(", "),
          ),
      })
      .collect(),
  }
}

/// What saving `body` as the version after `latest` (`None` for a new pipeline's first) needs a person to allow: the
/// approvals it drops, the tool steps it adds, and what it permits beyond `latest` ([`publish_reasons`]).
pub fn save_reasons(latest: Option<&Pipeline>, body: &Pipeline, assignables: &Assignables) -> Vec<Message> {
  let latest_steps = latest.map_or(&[][..], Pipeline::steps);
  let mut reasons = weakened_gates(latest_steps, body.steps());
  reasons.extend(added_tool_steps(latest_steps, body.steps(), assignables));
  for reason in publish_reasons(latest, body) {
    if !reasons.contains(&reason) {
      reasons.push(reason);
    }
  }
  reasons
}

/// The approvals `steps` drop from `base`, the latest published version: a step that waits for a person's approval,
/// or asks a person about its tool calls, and no longer does or is gone. Dropping one is a person's decision; other
/// narrowing within the profile is not.
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

/// The steps of `steps` that call a tool the latest published version (`base`) does not call under the same key: each
/// runs a program on this computer, which a person decides.
fn added_tool_steps(base: &[Step], steps: &[Step], assignables: &Assignables) -> Vec<Message> {
  let calls = |step: &Step| match step.kind {
    StepKind::Tool { tool } => tool,
    StepKind::Agent { .. } => None,
  };
  steps
    .iter()
    .filter_map(|step| Some((step, calls(step)?)))
    .filter(|(step, tool)| {
      !base
        .iter()
        .any(|before| before.key == step.key && calls(before) == Some(*tool))
    })
    .map(|(step, tool)| {
      let name = assignables
        .tool(tool)
        .map_or_else(|| tool.to_string(), |choice| choice.name.to_string());
      Message::new("appMcp.projectTools.widens.toolStep")
        .with("step", step.name.as_str().to_owned())
        .with("tool", name)
    })
    .collect()
}
