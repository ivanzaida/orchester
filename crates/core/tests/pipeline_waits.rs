//! What a step waits for when that is less than the whole stage drawn before it (`Pipelines · v2` P16, P16b), how a
//! step's *Waits for* changes it, and *Make it a line* for steps with no order to draw (P16c).
//! `cargo test -p orchester-core --test pipeline_waits`.

use std::num::NonZeroU32;

use orchester_core::{
  name::Name,
  resource::{
    FailureRoute, MoveError, MoveOffer, PartWait, PipelineEdit, PipelineError, PipelineSources, PromptText, Step,
    StepKey, StepKind, WaitChoice, Waited, flow_units, part_waits,
  },
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn keys(values: &[&str]) -> Vec<StepKey> {
  values.iter().map(|value| key(value)).collect()
}

fn step(name: &str, depends_on: &[&str]) -> Step {
  Step {
    key: key(name),
    name: Name::new(name).expect("valid name"),
    kind: StepKind::Tool { tool: None },
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn returning(mut step: Step, target: &str) -> Step {
  step.on_failure = Some(FailureRoute {
    return_to: key(target),
    max_retries: NonZeroU32::MIN,
  });
  step
}

fn edit(steps: Vec<Step>) -> PipelineEdit {
  PipelineEdit::from_steps("", PipelineSources::default(), steps)
}

/// P16's shape: research → plan → implement → { checks ∥ review } → notes, notes waiting for checks only.
fn branched() -> PipelineEdit {
  edit(vec![
    step("research", &[]),
    step("plan", &["research"]),
    step("implement", &["plan"]),
    step("checks", &["implement"]),
    step("review", &["implement"]),
    step("notes", &["checks"]),
  ])
}

fn waits_of(edit: &PipelineEdit, of: &str) -> Vec<String> {
  let mut waits: Vec<String> = edit
    .step(&key(of))
    .expect("the step")
    .depends_on
    .iter()
    .map(|key| key.as_str().to_owned())
    .collect();
  waits.sort();
  waits
}

fn choice<'a>(choices: &'a [WaitChoice], of: &str) -> &'a WaitChoice {
  choices
    .iter()
    .find(|choice| choice.key == key(of))
    .unwrap_or_else(|| panic!("{of} is offered"))
}

#[test]
fn a_step_that_waits_for_part_of_the_stage_before_says_what_it_waits_for_and_what_it_starts_before() {
  let edit = branched();
  let units = flow_units(edit.steps()).expect("the steps have a flow");
  assert_eq!(
    units.last(),
    Some(&keys(&["notes"])),
    "notes stands under the parallel group"
  );
  let waits = part_waits(edit.steps()).expect("the steps have a flow");
  assert_eq!(
    waits.get(&key("notes")),
    Some(&PartWait {
      on: keys(&["checks"]),
      not_on: keys(&["review"]),
    })
  );
  assert_eq!(waits.len(), 1, "every other step waits for the whole stage before it");
}

#[test]
fn independent_chains_each_say_which_step_they_wait_for() {
  let edit = edit(vec![
    step("fetch", &[]),
    step("lint", &[]),
    step("parse", &["fetch"]),
    step("report", &["lint"]),
  ]);
  let waits = part_waits(edit.steps()).expect("the steps have a flow");
  assert_eq!(waits[&key("parse")].not_on, keys(&["lint"]));
  assert_eq!(waits[&key("report")].not_on, keys(&["fetch"]));
}

#[test]
fn a_step_that_waits_for_the_stage_before_through_others_does_not_wait_partly() {
  // c waits for b, which waits for a: c is under b and waits for all of it.
  let edit = edit(vec![step("a", &[]), step("b", &["a"]), step("c", &["a", "b"])]);
  assert!(part_waits(edit.steps()).expect("a flow").is_empty());
}

#[test]
fn the_choices_leave_out_steps_that_wait_for_the_step_and_fade_those_it_waits_for_through_another() {
  let edit = branched();
  let choices = edit.wait_choices(&key("notes")).expect("the steps have a flow");
  let offered: Vec<&str> = choices.iter().map(|choice| choice.key.as_str()).collect();
  assert_eq!(offered, ["research", "plan", "implement", "checks", "review"]);
  for through in ["research", "plan", "implement"] {
    assert_eq!(
      choice(&choices, through).waited,
      Waited::Through(key("checks")),
      "{through}"
    );
    assert_eq!(choice(&choices, through).offer, MoveOffer::Unavailable);
  }
  assert_eq!(choice(&choices, "checks").waited, Waited::Directly);
  assert_eq!(choice(&choices, "review").waited, Waited::No);
  assert!(choice(&choices, "review").offer.is_available());

  let checks = edit.wait_choices(&key("checks")).expect("a flow");
  assert!(
    checks.iter().all(|choice| choice.key != key("notes")),
    "notes waits for checks, so checks cannot wait for it"
  );
}

#[test]
fn ticking_the_rest_of_the_stage_puts_the_step_back_under_all_of_it() {
  let mut edit = branched();
  assert_eq!(edit.wait_for(&key("notes"), &key("review"), true), Ok(true));
  assert_eq!(waits_of(&edit, "notes"), ["checks", "review"]);
  assert!(part_waits(edit.steps()).expect("a flow").is_empty());
  assert_eq!(
    edit.wait_for(&key("notes"), &key("review"), true),
    Ok(false),
    "it already waits for review"
  );
}

#[test]
fn ticking_a_later_step_drops_the_one_it_already_waits_for() {
  let mut edit = edit(vec![step("a", &[]), step("b", &["a"]), step("c", &["a"])]);
  assert_eq!(edit.wait_for(&key("c"), &key("b"), true), Ok(true));
  assert_eq!(waits_of(&edit, "c"), ["b"], "a is waited for through b");
}

#[test]
fn unticking_a_step_it_names_takes_it_out() {
  let mut edit = edit(vec![
    step("a", &[]),
    step("b", &["a"]),
    step("c", &["a"]),
    step("d", &["b", "c"]),
  ]);
  assert_eq!(edit.wait_for(&key("d"), &key("c"), false), Ok(true));
  assert_eq!(waits_of(&edit, "d"), ["b"]);
  assert_eq!(
    part_waits(edit.steps()).expect("a flow")[&key("d")].not_on,
    keys(&["c"])
  );
  assert_eq!(
    edit.wait_for(&key("d"), &key("a"), false),
    Ok(false),
    "d waits for a only through b"
  );
}

#[test]
fn waiting_for_a_step_that_waits_for_it_is_refused() {
  let mut edit = branched();
  let before = edit.clone();
  assert_eq!(
    edit.wait_for(&key("checks"), &key("notes"), true),
    Err(MoveError::WaitsForIt {
      step: key("checks"),
      dependency: key("notes"),
    })
  );
  assert_eq!(
    edit.wait_for(&key("checks"), &key("checks"), true),
    Err(MoveError::WaitsForIt {
      step: key("checks"),
      dependency: key("checks"),
    })
  );
  assert_eq!(edit, before);
}

#[test]
fn unticking_what_a_failure_route_needs_is_refused_and_offered_as_refused() {
  let mut edit = edit(vec![
    step("a", &[]),
    step("b", &["a"]),
    returning(step("c", &["b"]), "a"),
  ]);
  let refusal = MoveError::RetryTargetNotUpstream {
    step: key("c"),
    target: key("a"),
  };
  let choices = edit.wait_choices(&key("c")).expect("a flow");
  assert_eq!(choice(&choices, "b").offer, MoveOffer::Refused(refusal.clone()));
  let before = edit.clone();
  assert_eq!(edit.wait_for(&key("c"), &key("b"), false), Err(refusal));
  assert_eq!(edit, before);
}

#[test]
fn waiting_for_everything_before_it_puts_the_step_under_the_whole_stage() {
  let mut edit = branched();
  assert_eq!(edit.wait_for_stage_before(&key("notes")), Ok(true));
  assert_eq!(waits_of(&edit, "notes"), ["checks", "review"]);
  assert_eq!(
    edit.wait_for_stage_before(&key("notes")),
    Ok(false),
    "nothing left to wait for"
  );
  assert_eq!(
    edit.wait_for_stage_before(&key("research")),
    Ok(false),
    "nothing runs before the first stage"
  );
  assert_eq!(
    edit.wait_for_stage_before(&key("gone")),
    Err(MoveError::UnknownStep(key("gone")))
  );
}

#[test]
fn a_flow_with_no_order_offers_no_choices() {
  let edit = edit(vec![step("a", &["b"]), step("b", &["a"])]);
  assert!(matches!(
    edit.wait_choices(&key("a")),
    Err(MoveError::Broken(PipelineError::Cycle(_)))
  ));
}

#[test]
fn making_it_a_line_has_every_step_wait_for_the_one_listed_before_it() {
  let mut edit = edit(vec![
    step("research", &[]),
    step("review", &["notes"]),
    step("notes", &["review"]),
    step("gone", &["missing"]),
  ]);
  assert!(edit.validate().is_err());
  assert!(edit.line_fixes());
  assert!(edit.make_line());
  assert_eq!(waits_of(&edit, "research"), Vec::<String>::new());
  assert_eq!(waits_of(&edit, "review"), ["research"]);
  assert_eq!(waits_of(&edit, "notes"), ["review"]);
  assert_eq!(waits_of(&edit, "gone"), ["notes"]);
  edit.validate().expect("a line breaks no rule");
  assert!(!edit.make_line(), "a line stays as it is");
}

#[test]
fn a_line_does_not_fix_two_steps_with_one_key() {
  let edit = edit(vec![step("a", &[]), step("a", &[])]);
  assert!(!edit.line_fixes());
}

#[test]
fn a_line_does_not_fix_a_failure_route_to_a_later_step() {
  let edit = edit(vec![returning(step("a", &["b"]), "b"), step("b", &["a"])]);
  assert!(!edit.line_fixes(), "a would return to b, which runs after it");
}

#[test]
fn a_step_that_waits_partly_still_moves() {
  let mut edit = branched();
  assert_eq!(edit.move_up(&key("notes")), Ok(true));
  edit.validate().expect("the moved steps keep the pipeline rules");
}
