use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    MoveError, MoveOffer, ParallelChoice, PipelineEdit, PipelineError, PipelineSources, PromptText, StarterAssignments,
    StarterStep, StarterStepText, Step, StepKey, StepKind, feature_delivery,
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

/// a → b → c → d
fn linear() -> PipelineEdit {
  PipelineEdit::from_steps(
    "",
    PipelineSources::default(),
    vec![step("a", &[]), step("b", &["a"]), step("c", &["b"]), step("d", &["c"])],
  )
}

fn text(starter: StarterStep) -> StarterStepText {
  StarterStepText {
    name: Name::new(starter.key()).expect("valid name"),
    instructions: PromptText::new("Work on {{task.title}}.").expect("valid prompt"),
  }
}

fn assigned() -> StarterAssignments {
  StarterAssignments {
    explorer: Some(ResourceId::generate()),
    planner: Some(ResourceId::generate()),
    builder: Some(ResourceId::generate()),
    reviewer: Some(ResourceId::generate()),
    checks: Some(ResourceId::generate()),
    pull_request: Some(ResourceId::generate()),
  }
}

/// 01 research → 02 plan (gate) → 03 implement → { 04 run-checks ∥ 05 review } → 06 create-pull-request; the
/// checks and the review return to the implementation when they fail.
fn feature() -> PipelineEdit {
  PipelineEdit::new(feature_delivery(text, assigned()))
}

/// The steps in their order, each with what it waits for.
fn flow(edit: &PipelineEdit) -> Vec<(&str, Vec<&str>)> {
  edit
    .steps()
    .iter()
    .map(|step| {
      (
        step.key.as_str(),
        step.depends_on.iter().map(StepKey::as_str).collect::<Vec<_>>(),
      )
    })
    .collect()
}

/// Every step as it was, but for what it waits for, and the edit still a valid pipeline.
fn assert_only_the_flow_changed(before: &PipelineEdit, after: &PipelineEdit) {
  assert_eq!(before.steps().len(), after.steps().len());
  for step in before.steps() {
    let now = after.step(&step.key).expect("no step is lost");
    let flat = |step: &Step| Step {
      depends_on: Default::default(),
      ..step.clone()
    };
    assert_eq!(flat(step), flat(now), "{} changed more than its place", step.key);
  }
  after.validate().expect("the moved pipeline keeps the pipeline rules");
}

fn retry_refusal(step: &str, target: &str) -> MoveError {
  MoveError::RetryTargetNotUpstream {
    step: key(step),
    target: key(target),
  }
}

#[test]
fn a_linear_step_moves_up_and_the_steps_around_it_keep_their_order() {
  let mut edit = linear();
  let before = edit.clone();
  assert_eq!(edit.move_up(&key("c")), Ok(true));
  assert_eq!(
    flow(&edit),
    [("a", vec![]), ("c", vec!["a"]), ("b", vec!["c"]), ("d", vec!["b"])]
  );
  assert_only_the_flow_changed(&before, &edit);
}

#[test]
fn a_linear_step_moves_down() {
  let mut edit = linear();
  assert_eq!(edit.move_down(&key("a")), Ok(true));
  assert_eq!(
    flow(&edit),
    [("b", vec![]), ("a", vec!["b"]), ("c", vec!["a"]), ("d", vec!["c"])]
  );
}

#[test]
fn the_first_step_cannot_move_up_and_the_last_cannot_move_down() {
  let mut edit = linear();
  let before = edit.clone();
  assert_eq!(edit.move_up(&key("a")), Ok(false));
  assert_eq!(edit.move_down(&key("d")), Ok(false));
  assert_eq!(edit, before);
}

#[test]
fn a_linear_step_lands_between_two_places() {
  let mut edit = linear();
  assert_eq!(edit.move_to(&key("d"), Some(&key("b"))), Ok(true));
  assert_eq!(
    flow(&edit),
    [("a", vec![]), ("d", vec!["a"]), ("b", vec!["d"]), ("c", vec!["b"])]
  );
  assert_eq!(edit.move_to(&key("a"), None), Ok(true));
  assert_eq!(
    flow(&edit),
    [("d", vec![]), ("b", vec!["d"]), ("c", vec!["b"]), ("a", vec!["c"])]
  );
}

#[test]
fn dropping_a_step_where_it_already_is_changes_nothing() {
  let mut edit = linear();
  let before = edit.clone();
  assert_eq!(edit.move_to(&key("b"), Some(&key("c"))), Ok(false));
  assert_eq!(edit.move_to(&key("b"), Some(&key("b"))), Ok(false));
  assert_eq!(edit.move_to(&key("d"), None), Ok(false));
  assert_eq!(edit, before);
}

#[test]
fn a_linear_step_runs_in_parallel_with_a_neighbour() {
  let mut edit = linear();
  let before = edit.clone();
  assert_eq!(edit.run_in_parallel_with(&key("c"), &key("a")), Ok(true));
  assert_eq!(
    flow(&edit),
    [("a", vec![]), ("c", vec![]), ("b", vec!["a", "c"]), ("d", vec!["b"])]
  );
  assert_only_the_flow_changed(&before, &edit);

  let mut edit = linear();
  assert_eq!(edit.run_in_parallel_with(&key("b"), &key("d")), Ok(true));
  assert_eq!(
    flow(&edit),
    [("a", vec![]), ("c", vec!["a"]), ("b", vec!["c"]), ("d", vec!["c"])]
  );
}

#[test]
fn steps_already_side_by_side_are_left_alone_and_a_step_cannot_join_itself() {
  let mut edit = linear();
  edit.run_in_parallel_with(&key("c"), &key("a")).expect("allowed");
  let before = edit.clone();
  assert_eq!(edit.run_in_parallel_with(&key("a"), &key("c")), Ok(false));
  assert_eq!(
    edit.run_in_parallel_with(&key("a"), &key("a")),
    Err(MoveError::WithItself(key("a")))
  );
  assert_eq!(
    edit.run_in_parallel_with(&key("a"), &key("zzz")),
    Err(MoveError::UnknownStep(key("zzz")))
  );
  assert_eq!(edit, before);
}

#[test]
fn a_step_leaves_its_group_and_runs_right_after_it() {
  let mut edit = linear();
  edit.run_in_parallel_with(&key("c"), &key("a")).expect("allowed");
  assert_eq!(edit.run_on_its_own(&key("c")), Ok(true));
  assert_eq!(
    flow(&edit),
    [("a", vec![]), ("c", vec!["a"]), ("b", vec!["c"]), ("d", vec!["b"])]
  );
  assert_eq!(edit.run_on_its_own(&key("c")), Ok(false));
}

#[test]
fn unknown_steps_and_broken_pipelines_are_reported() {
  let mut edit = linear();
  assert_eq!(edit.move_up(&key("zzz")), Err(MoveError::UnknownStep(key("zzz"))));
  assert_eq!(
    edit.possible_moves(&key("zzz")),
    Err(MoveError::UnknownStep(key("zzz")))
  );
  assert!(edit.set_dependency(&key("a"), &key("d"), true));
  assert!(matches!(
    edit.move_up(&key("c")),
    Err(MoveError::Broken(PipelineError::Cycle(_)))
  ));
}

#[test]
fn the_starter_has_the_shape_of_the_design() {
  assert_eq!(
    flow(&feature()),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("run-checks", vec!["implement"]),
      ("review", vec!["implement"]),
      ("create-pull-request", vec!["review", "run-checks"]),
    ]
  );
}

#[test]
fn a_step_trades_places_with_a_neighbour_before_a_group() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(edit.move_up(&key("plan")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("plan", vec![]),
      ("research", vec!["plan"]),
      ("implement", vec!["research"]),
      ("run-checks", vec!["implement"]),
      ("review", vec!["implement"]),
      ("create-pull-request", vec!["review", "run-checks"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);
  assert!(edit.step(&key("plan")).expect("step").approval_gate);
}

#[test]
fn a_step_trades_places_with_a_whole_group() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(edit.move_up(&key("create-pull-request")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("create-pull-request", vec!["implement"]),
      ("run-checks", vec!["create-pull-request"]),
      ("review", vec!["create-pull-request"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);
}

#[test]
fn a_step_of_a_group_leaves_it_when_it_moves_down() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(edit.move_down(&key("review")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("run-checks", vec!["implement"]),
      ("create-pull-request", vec!["run-checks"]),
      ("review", vec!["create-pull-request"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);
}

#[test]
fn a_move_that_puts_a_failure_route_downstream_is_refused_with_the_reason() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(
    edit.move_up(&key("run-checks")),
    Err(retry_refusal("run-checks", "implement"))
  );
  assert_eq!(
    edit.move_down(&key("implement")),
    Err(retry_refusal("run-checks", "implement"))
  );
  assert_eq!(
    edit.move_to(&key("review"), Some(&key("plan"))),
    Err(retry_refusal("review", "implement"))
  );
  assert_eq!(
    edit.run_in_parallel_with(&key("review"), &key("plan")),
    Err(retry_refusal("review", "implement"))
  );
  assert_eq!(
    edit.run_in_parallel_with(&key("implement"), &key("review")),
    Err(retry_refusal("run-checks", "implement"))
  );
  assert_eq!(edit, before, "a refused move changes nothing");
}

#[test]
fn a_step_joins_a_neighbour_and_the_steps_after_it_wait_for_it() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(edit.run_in_parallel_with(&key("plan"), &key("implement")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["research"]),
      ("run-checks", vec!["implement", "plan"]),
      ("review", vec!["implement", "plan"]),
      ("create-pull-request", vec!["review", "run-checks"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);
}

#[test]
fn a_step_joins_a_parallel_group() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(
    edit.run_in_parallel_with(&key("create-pull-request"), &key("review")),
    Ok(true)
  );
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("run-checks", vec!["implement"]),
      ("review", vec!["implement"]),
      ("create-pull-request", vec!["implement"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);
}

#[test]
fn a_step_of_a_group_runs_on_its_own() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(edit.run_on_its_own(&key("review")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("run-checks", vec!["implement"]),
      ("review", vec!["run-checks"]),
      ("create-pull-request", vec!["review"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);

  let mut edit = feature();
  assert_eq!(edit.run_on_its_own(&key("run-checks")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("review", vec!["implement"]),
      ("run-checks", vec!["review"]),
      ("create-pull-request", vec!["run-checks"]),
    ]
  );
  assert_eq!(edit.run_on_its_own(&key("plan")), Ok(false));
}

#[test]
fn a_step_is_dropped_between_two_places_of_the_flow() {
  let mut edit = feature();
  let before = edit.clone();
  assert_eq!(
    edit.move_to(&key("research"), Some(&key("create-pull-request"))),
    Ok(true)
  );
  assert_eq!(
    flow(&edit),
    [
      ("plan", vec![]),
      ("implement", vec!["plan"]),
      ("run-checks", vec!["implement"]),
      ("review", vec!["implement"]),
      ("research", vec!["review", "run-checks"]),
      ("create-pull-request", vec!["research"]),
    ]
  );
  assert_only_the_flow_changed(&before, &edit);

  let mut edit = feature();
  assert_eq!(edit.move_to(&key("review"), Some(&key("run-checks"))), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("review", vec!["implement"]),
      ("run-checks", vec!["review"]),
      ("create-pull-request", vec!["run-checks"]),
    ]
  );
}

#[test]
fn a_step_of_a_group_is_dropped_at_the_end() {
  let mut edit = feature();
  assert_eq!(edit.move_to(&key("run-checks"), None), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("review", vec!["implement"]),
      ("create-pull-request", vec!["review"]),
      ("run-checks", vec!["create-pull-request"]),
    ]
  );
}

fn parallel(with: &[&str], offer: MoveOffer) -> ParallelChoice {
  ParallelChoice {
    with: keys(with),
    offer,
  }
}

#[test]
fn the_first_step_offers_no_move_up_and_one_neighbour() {
  let moves = feature().possible_moves(&key("research")).expect("moves");
  assert_eq!(moves.up, MoveOffer::Unavailable);
  assert_eq!(moves.down, MoveOffer::Available);
  assert_eq!(moves.on_its_own, MoveOffer::Unavailable);
  assert_eq!(moves.parallel_with, [parallel(&["plan"], MoveOffer::Available)]);
}

#[test]
fn the_last_step_offers_no_move_down_and_the_group_before_it() {
  let moves = feature().possible_moves(&key("create-pull-request")).expect("moves");
  assert_eq!(moves.up, MoveOffer::Available);
  assert_eq!(moves.down, MoveOffer::Unavailable);
  assert_eq!(moves.on_its_own, MoveOffer::Unavailable);
  assert_eq!(
    moves.parallel_with,
    [parallel(&["run-checks", "review"], MoveOffer::Available)]
  );
}

#[test]
fn a_step_of_a_group_offers_leaving_it_and_says_why_a_move_is_refused() {
  let moves = feature().possible_moves(&key("run-checks")).expect("moves");
  assert_eq!(moves.up, MoveOffer::Refused(retry_refusal("run-checks", "implement")));
  assert_eq!(moves.down, MoveOffer::Available);
  assert_eq!(moves.on_its_own, MoveOffer::Available);
  assert_eq!(
    moves.parallel_with,
    [
      parallel(
        &["implement"],
        MoveOffer::Refused(retry_refusal("run-checks", "implement"))
      ),
      parallel(&["create-pull-request"], MoveOffer::Available),
    ]
  );
  assert!(moves.down.is_available() && !moves.up.is_available());
}

#[test]
fn a_step_in_the_middle_offers_both_neighbours_and_a_group() {
  let moves = feature().possible_moves(&key("implement")).expect("moves");
  assert_eq!(moves.up, MoveOffer::Available);
  assert_eq!(moves.down, MoveOffer::Refused(retry_refusal("run-checks", "implement")));
  assert_eq!(moves.on_its_own, MoveOffer::Unavailable);
  assert_eq!(
    moves.parallel_with,
    [
      parallel(&["plan"], MoveOffer::Available),
      parallel(
        &["run-checks", "review"],
        MoveOffer::Refused(retry_refusal("run-checks", "implement"))
      ),
    ]
  );
}

#[test]
fn every_offered_move_can_be_made() {
  let edit = feature();
  for step in edit.steps() {
    let moves = edit.possible_moves(&step.key).expect("moves");
    let mut tries = vec![(moves.up, 0), (moves.down, 1), (moves.on_its_own, 2)];
    tries.extend(
      moves
        .parallel_with
        .iter()
        .enumerate()
        .map(|(index, choice)| (choice.offer.clone(), 3 + index)),
    );
    for (offer, which) in tries {
      let mut tried = edit.clone();
      let neighbour = |index: usize| moves.parallel_with[index - 3].with[0].clone();
      let result = match which {
        0 => tried.move_up(&step.key),
        1 => tried.move_down(&step.key),
        2 => tried.run_on_its_own(&step.key),
        index => tried.run_in_parallel_with(&step.key, &neighbour(index)),
      };
      match offer {
        MoveOffer::Available => {
          assert_eq!(result, Ok(true), "{} move {which}", step.key);
          tried.validate().expect("valid");
        }
        MoveOffer::Unavailable => assert_eq!(result, Ok(false), "{} move {which}", step.key),
        MoveOffer::Refused(error) => assert_eq!(result, Err(error), "{} move {which}", step.key),
      }
    }
  }
}

#[test]
fn a_duplicated_step_is_a_member_of_its_group_when_the_steps_around_it_move() {
  let mut edit = feature();
  let copy = edit.duplicate_step(&key("review")).expect("review exists");
  assert_eq!(copy.as_str(), "review-copy");
  assert_eq!(edit.move_up(&key("create-pull-request")), Ok(true));
  assert_eq!(
    flow(&edit),
    [
      ("research", vec![]),
      ("plan", vec!["research"]),
      ("implement", vec!["plan"]),
      ("create-pull-request", vec!["implement"]),
      ("run-checks", vec!["create-pull-request"]),
      ("review", vec!["create-pull-request"]),
      ("review-copy", vec!["create-pull-request"]),
    ]
  );
  edit.validate().expect("valid");
}
