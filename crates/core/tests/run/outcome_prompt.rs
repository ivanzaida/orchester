//! The outcome contract must reach new and existing sessions, including schema-free resumes.

use orchester_core::{
  resource::InjectionEvent,
  run::{DeliveredMessages, MessageOccasion, PlanRevision, PlanVersion, ReviewNote, STEP_RESULT_SCHEMA},
};

use crate::{
  fixture::{Config, key, task},
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
    "Use `failed` for checks that require corrections by an earlier step",
    "action by the person would let you continue",
    "do not use it merely because work is incomplete or needs permission",
    "If the person already answered the relevant question, apply their answer",
    "current step is complete and its required checks pass",
    "completing the inspection alone is not success",
    "unresolved blocking or major findings requires `failed`",
    "do not repair them in a read-only review",
    "verdicts must describe the current inputs",
    "Earlier failures are context, not a verdict on changed work",
    "Reuse a check only when its inputs are unchanged",
    "Preserve earlier evidence without relabeling it as evidence for a new build",
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
    serde_json::json!(["done", "failed", "unverified", "question"])
  );
  // The handoff lists are required but nullable: Codex's structured output requires every property.
  assert_eq!(
    schema["required"],
    serde_json::json!([
      "outcome",
      "summary",
      "unverified",
      "completed",
      "decisions",
      "open_questions",
      "next_agent_should_know"
    ])
  );
  assert_eq!(schema["additionalProperties"], false);
  let description = schema["properties"]["outcome"]["description"]
    .as_str()
    .expect("guidance");
  assert!(description.contains("question: waiting for the person's decision, permission, access, or clarification"));
  assert!(description.contains("failed: failure handled by the pipeline"));
  assert!(description.contains("required checks pass"));
  assert!(description.contains("negative verdict or unresolved blocking or major findings is failed"));
  assert!(description.contains("corrections by an earlier step"));
}

#[test]
fn outcome_prompt_unverified_is_only_for_this_steps_own_checks_and_names_the_later_steps() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let started = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt");
  let expected = contract(&started.text);
  for rule in [
    "Use `unverified` only for checks this step itself is asked to do and could not",
    "A check that a later step of this run's pipeline does",
    "is not unverified here: leave it to that step, name it in your summary as left to that step, and do not list \
     it in `unverified`",
    "Mismatches you did find still require `failed`",
    // Implement of Feature delivery comes before checks, review and pr, in pipeline order.
    "The steps after this one in this run's pipeline: checks (`checks`), review (`review`), pr (`pr`).",
  ] {
    assert!(expected.contains(rule), "missing rule: {rule}\n{expected}");
  }
  let resumed = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Resume)
    .expect("prompt");
  assert_eq!(contract(&resumed.text), expected);
  // A preview has no run, so it names no steps.
  let preview = prompt.input(&[], &[], false).compose(InjectionEvent::Start);
  assert!(
    !preview.text().contains("The steps after this one"),
    "{}",
    preview.text()
  );
}

#[test]
fn outcome_prompt_the_last_step_is_told_its_checks_are_its_own() {
  let pr = Config::feature_delivery()
    .pipeline_version
    .body()
    .step(&key("pr"))
    .expect("step")
    .clone();
  let prompt = Prompt::new(builder("Role."), pr, task("Brief.", &[]));
  let text = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt")
    .text;
  assert!(
    contract(&text).contains("No step of this run's pipeline comes after this one"),
    "{text}"
  );
}

#[test]
fn outcome_prompt_schema_keeps_later_steps_checks_out_of_unverified() {
  let schema: serde_json::Value = serde_json::from_str(STEP_RESULT_SCHEMA).expect("valid schema");
  let outcome = schema["properties"]["outcome"]["description"]
    .as_str()
    .expect("guidance");
  assert!(outcome.contains("some of what this step itself must check could not be checked"));
  assert!(outcome.contains("A check a later step of the pipeline does is not unverified"));
  let list = schema["properties"]["unverified"]["description"]
    .as_str()
    .expect("guidance");
  assert!(list.contains("never a check a later step does"));
}
