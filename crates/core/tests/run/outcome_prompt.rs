//! The outcome contract must reach new and existing sessions, including schema-free resumes.

use orchester_core::{
  resource::{InjectionEvent, Pipeline, PromptText, Step},
  run::{DeliveredMessages, MessageOccasion, PlanRevision, PlanVersion, ReviewNote, STEP_RESULT_SCHEMA},
};

use crate::{
  fixture::{Config, Logged, at, key, task, version},
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
fn outcome_prompt_a_check_this_step_was_asked_for_stays_its_own() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let started = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt");
  let expected = contract(&started.text);
  for rule in [
    "Use `done` when your current step is complete and its required checks pass",
    "A check this step's own instructions require is this step's, even if a later step may run it again",
    "use `unverified` and list it with what it is and why",
    "Mismatches you did find still require `failed`",
    "Only a check this step was not asked to do may be left to a later step",
    "only to one whose instructions show it does that check: name that step in your summary",
    "A check this step was asked to do stays this step's, whatever a later step does.",
  ] {
    assert!(expected.contains(rule), "missing rule: {rule}\n{expected}");
  }
  // No list of checks a later step might do invites a guess.
  assert!(!expected.contains("design verification"), "{expected}");
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

/// The instructions [`later_steps_prompt`] gives pr.
const PR: &str = "Open a pull request for {{run.branch}}! Then wait.";

/// Feature delivery with instructions for review, and `pr` for pr, and a prompt for its `step` in a started run of
/// it.
fn later_steps_prompt(step: &str, pr: &str) -> Prompt {
  let mut config = Config::feature_delivery();
  let steps: Vec<Step> = config
    .pipeline_version
    .body()
    .steps()
    .iter()
    .cloned()
    .map(|mut step| {
      let instructions = match step.key.as_str() {
        "review" => "# Review\n\nReview the change against the brief and run the tests. Report each finding.",
        "pr" => pr,
        _ => return step,
      };
      step.instructions = PromptText::new(instructions).expect("valid prompt");
      step
    })
    .collect();
  config.pipeline_version = version(3, Pipeline::new(steps).expect("valid pipeline"));
  let own = config.pipeline_version.body().step(&key(step)).expect("step").clone();
  let mut prompt = Prompt::new(builder("Role."), own, task("Brief.", &[]));
  let mut logged = Logged::pending_with(&config);
  logged.apply(|run| run.start(at(11)));
  prompt.logged = logged;
  prompt
}

fn contract_of(prompt: &Prompt) -> String {
  let text = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("prompt")
    .text;
  contract(&text).to_owned()
}

#[test]
fn outcome_prompt_names_the_steps_after_this_one_with_the_start_of_their_instructions() {
  let implement = contract_of(&later_steps_prompt("implement", PR));
  // Implement of Feature delivery comes before checks, review and pr, in pipeline order; a heading is skipped and
  // only the first sentence shows; placeholders stay as written.
  let list = [
    "The steps after this one in this run's pipeline, each with the start of its instructions:",
    "- checks (`checks`): (no instructions)",
    "- review (`review`): Review the change against the brief and run the tests.",
    "- pr (`pr`): Open a pull request for {{run.branch}}!",
    "Leave a check to one of them only when this step was not asked to do it",
  ]
  .join("\n");
  assert!(implement.contains(&list), "{implement}");
  assert!(!implement.contains("Report each finding"), "{implement}");
}

#[test]
fn outcome_prompt_lists_only_the_steps_this_one_comes_before() {
  // checks and review both follow implement; only pr depends on checks.
  let checks = contract_of(&later_steps_prompt("checks", PR));
  assert!(checks.contains("- pr (`pr`): Open a pull request"), "{checks}");
  assert!(!checks.contains("(`review`)"), "{checks}");
  assert!(!checks.contains("(`checks`)"), "{checks}");
}

#[test]
fn outcome_prompt_the_last_step_can_leave_no_check_to_a_later_step() {
  let pr = contract_of(&later_steps_prompt("pr", PR));
  assert!(
    pr.contains("No step of this run's pipeline comes after this one: no check can be left to a later step."),
    "{pr}"
  );
  assert!(!pr.contains("The steps after this one"), "{pr}");
}

#[test]
fn outcome_prompt_a_long_first_sentence_is_cut() {
  let long = format!("{} end.", "word ".repeat(60));
  let checks = contract_of(&later_steps_prompt("checks", &long));
  let line = checks
    .lines()
    .find(|line| line.starts_with("- pr (`pr`): "))
    .expect("pr is listed");
  assert!(line.ends_with('…'), "{line}");
  // 160 characters of the sentence, its trailing space dropped, then the mark.
  assert_eq!(line.trim_start_matches("- pr (`pr`): ").chars().count(), 160, "{line}");
}

#[test]
fn outcome_prompt_schema_keeps_a_steps_own_checks_in_unverified() {
  let schema: serde_json::Value = serde_json::from_str(STEP_RESULT_SCHEMA).expect("valid schema");
  let outcome = schema["properties"]["outcome"]["description"]
    .as_str()
    .expect("guidance");
  assert!(outcome.contains("done: current step complete and required checks pass"));
  assert!(outcome.contains("a check its own instructions require could not be done"));
  assert!(outcome.contains("even if a later step may run it again"));
  assert!(outcome.contains("Only a check the step was not asked to do may be left to a later step"));
  let list = schema["properties"]["unverified"]["description"]
    .as_str()
    .expect("guidance");
  assert!(list.contains("each check the step's own instructions require that it could not do"));
}
