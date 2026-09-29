//! The outcome contract must reach new and existing sessions, including schema-free resumes.

use orchester_core::{
  resource::InjectionEvent,
  run::{DeliveredMessages, MessageOccasion, PlanRevision, PlanVersion, ReviewNote, STEP_RESULT_SCHEMA},
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

fn contract(text: &str) -> &str {
  let (context, contract) = text.split_once("# When you finish\n").expect("outcome contract");
  assert!(!context.contains("End with exactly one JSON object"));
  assert!(!contract.contains("# When you finish"), "contract included only once");
  contract
}

#[test]
fn outcome_prompt_keeps_permission_and_scope_decisions_resumable_on_every_turn() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let started = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt");
  let expected = contract(&started.text);
  for rule in [
    "exactly one JSON object containing `outcome` and `summary`",
    "Use `question` when progress needs the person's decision, permission, access, or clarification",
    "a proposed command outside the sandbox",
    "changing scope",
    "ask a concrete question with the available options",
    "resumes the same session",
    "Requesting permission does not grant it",
    "Use `failed` only for a failure",
    "no decision or action by the person would let you continue",
    "do not use it merely because work is incomplete or needs permission",
    "If the person already answered the relevant question, apply their answer",
  ] {
    assert!(expected.contains(rule), "missing rule: {rule}");
  }
  // No messages covers pause/permission checkpoint resumes; the others cover conversation paths.
  for occasion in [
    None,
    Some(MessageOccasion::Written),
    Some(MessageOccasion::AfterTurn),
    Some(MessageOccasion::Answer {
      question: "Allow this command outside the sandbox?",
    }),
    Some(MessageOccasion::FollowUp),
    Some(MessageOccasion::Recovery {
      previous_result: "Saved partial work.",
    }),
  ] {
    let mut input = prompt.input(&[], &[], true);
    input.messages = occasion.map(|occasion| DeliveredMessages {
      occasion,
      messages: &[],
    });
    let resumed = input.assemble(InjectionEvent::Resume).expect("prompt");
    assert_eq!(contract(&resumed.text), expected);
  }
}

#[test]
fn outcome_prompt_revision_keeps_the_contract_and_complete_plan_requirement() {
  let mut step = step_with("Step.");
  step.approval_gate = true;
  let prompt = Prompt::new(builder("Role."), step, task("Brief.", &[]));
  let started = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt");
  let note = ReviewNote::new("Ask before changing scope.").expect("note");
  let mut input = prompt.input(&[], &[], true);
  input.revision = Some(PlanRevision {
    version: PlanVersion::FIRST,
    plan: "Original plan.",
    note: &note,
  });
  let revised = input.assemble(InjectionEvent::Resume).expect("prompt");
  assert_eq!(contract(&revised.text), contract(&started.text));
  assert!(revised.text.contains("make it the complete result of this step"));
}

#[test]
fn outcome_prompt_schema_describes_attention_without_changing_the_wire_format() {
  let schema: serde_json::Value = serde_json::from_str(STEP_RESULT_SCHEMA).expect("valid schema");
  assert_eq!(
    schema["properties"]["outcome"]["enum"],
    serde_json::json!(["done", "failed", "question"])
  );
  assert_eq!(schema["required"], serde_json::json!(["outcome", "summary"]));
  assert_eq!(schema["additionalProperties"], false);
  let description = schema["properties"]["outcome"]["description"]
    .as_str()
    .expect("guidance");
  assert!(description.contains("question: waiting for the person's decision, permission, access, or clarification"));
  assert!(description.contains("failed: failure handled by the pipeline"));
}
