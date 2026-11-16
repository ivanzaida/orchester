//! A module with a branch of its own on its way to its base branch (`orchester_core::module::delivery`, OA-17, OA-20):
//! the name its branch gets, what it waits for once its every task is done, what its standing raises then, and what
//! its acceptance is told.
//! `cargo test -p orchester-core --test module_delivery delivery`.

use orchester_core::{
  id::ModuleId,
  module::delivery::{ACCEPTANCES_MOST, Acceptance, DeliveryFacts, DeliveryStep, PersonsDelivery, own_branch_name},
  name::Name,
  orchestrator::{
    Ceilings, WatchTarget,
    instructions::{ACCEPTANCE_CRITERIA, AcceptedTask, acceptance_brief, acceptance_title, standing_prompt},
    standing::{Around, Raised, StandingEvent, TargetStanding, TaskFacts, standing},
  },
  run::{BranchName, TaskPhase},
};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("branch")
}

#[test]
fn delivery_a_module_gets_a_branch_named_by_its_slug_that_nothing_has() {
  let named = |name: &str, taken: &[&str]| {
    let taken: Vec<BranchName> = taken.iter().map(|taken| branch(taken)).collect();
    own_branch_name(&Name::new(name).expect("name"), |candidate| taken.contains(candidate))
      .expect("a name")
      .as_str()
      .to_owned()
  };
  assert_eq!(named("Webhooks v2", &[]), "module/webhooks-v2");
  assert_eq!(named("  Billing & Invoices!  ", &[]), "module/billing-invoices");
  assert_eq!(named("Модуль", &[]), "module/module", "a name with no letter git takes");
  assert_eq!(named("Webhooks v2", &["module/webhooks-v2"]), "module/webhooks-v2-2");
  assert_eq!(
    named("Webhooks v2", &["module/webhooks-v2", "module/webhooks-v2-2"]),
    "module/webhooks-v2-3"
  );
}

fn facts(acceptance: Acceptance) -> DeliveryFacts {
  DeliveryFacts {
    own_branch: true,
    acceptance_pipeline: true,
    deliver_when_accepted: true,
    acceptances: 0,
    acceptance,
    delivered: false,
  }
}

#[test]
fn delivery_a_module_waits_for_its_acceptance_then_its_delivery() {
  let none = facts(Acceptance::None);
  assert_eq!(none.step(), DeliveryStep::Accept);
  assert_eq!(facts(Acceptance::UnderWay).step(), DeliveryStep::Accepting);
  assert_eq!(facts(Acceptance::Passed).step(), DeliveryStep::Deliver);
  // Work merged after an acceptance, or one that failed, asks for another, as many as the chat starts by itself.
  let again = DeliveryFacts {
    acceptances: 1,
    ..facts(Acceptance::NotPassed)
  };
  assert_eq!(again.step(), DeliveryStep::Accept);
  let spent = DeliveryFacts {
    acceptances: ACCEPTANCES_MOST,
    ..facts(Acceptance::NotPassed)
  };
  assert_eq!(spent.step(), DeliveryStep::Person(PersonsDelivery::AcceptanceCeiling));
  // What only the person does.
  let unguarded = DeliveryFacts {
    acceptance_pipeline: false,
    ..none
  };
  assert_eq!(unguarded.step(), DeliveryStep::Person(PersonsDelivery::NoAcceptance));
  let asked = DeliveryFacts {
    deliver_when_accepted: false,
    ..facts(Acceptance::Passed)
  };
  assert_eq!(asked.step(), DeliveryStep::Person(PersonsDelivery::NotAllowed));
  // Nothing to wait for.
  let delivered = DeliveryFacts {
    delivered: true,
    ..none
  };
  assert_eq!(delivered.step(), DeliveryStep::Delivered);
  let plain = DeliveryFacts {
    own_branch: false,
    ..none
  };
  assert_eq!(plain.step(), DeliveryStep::Delivered);
}

fn done(key: &str) -> TaskFacts {
  TaskFacts {
    key: key.to_owned(),
    closed: true,
    phase: TaskPhase::Merged,
    runs: 1,
    unready: None,
    parked: None,
    blockers: Vec::new(),
    low_disk: false,
    held: false,
  }
}

fn around(delivery: Option<DeliveryStep>) -> Around {
  Around {
    free_slots: 2,
    ceilings: Some(Ceilings::default()),
    budget_used: false,
    delivery,
  }
}

#[test]
fn delivery_a_module_whose_tasks_are_done_stands_to_deliver_until_it_is_delivered() {
  let tasks = [done("KON-1"), done("KON-2")];
  let stands = |step| standing(&tasks, &[], &around(step));
  assert_eq!(stands(None).target, TargetStanding::Done);
  assert_eq!(stands(Some(DeliveryStep::Delivered)).target, TargetStanding::Done);
  let accept = stands(Some(DeliveryStep::Accept));
  assert_eq!(accept.target, TargetStanding::ToDeliver(DeliveryStep::Accept));
  assert_eq!(accept.target.name(), "to_deliver");
  let raised = |step| Raised::of(&stands(Some(step)), |_| None);
  assert_eq!(raised(DeliveryStep::Accept), Some(Raised::Accept));
  assert_eq!(raised(DeliveryStep::Deliver), Some(Raised::Deliver));
  assert_eq!(
    raised(DeliveryStep::Accepting),
    None,
    "its acceptance runs: nothing to do"
  );
  let waits = raised(DeliveryStep::Person(PersonsDelivery::NotAllowed)).expect("raised");
  assert_eq!(
    waits,
    Raised::DeliveryWaits {
      why: PersonsDelivery::NotAllowed
    }
  );
  // Each state is told once: they differ.
  let states: Vec<String> = [Raised::Accept, Raised::Deliver, waits]
    .iter()
    .map(Raised::state)
    .collect();
  assert_eq!(states, ["accept", "deliver", "delivery_waits:person_not_allowed"]);
  // A task still open keeps the module where its tasks are.
  let mut open = done("KON-3");
  (open.closed, open.phase) = (false, TaskPhase::Running);
  let working = standing(&[done("KON-1"), open], &[], &around(Some(DeliveryStep::Accept)));
  assert_eq!(working.target, TargetStanding::Working);
}

#[test]
fn delivery_the_turn_of_an_acceptance_and_of_a_delivery_is_told_what_to_call() {
  let event = |raised| StandingEvent {
    target: WatchTarget::Module(ModuleId::generate()),
    name: Some("Webhooks v2".to_owned()),
    raised,
    automatic: true,
  };
  let accept = standing_prompt(&event(Raised::Accept));
  assert!(
    accept.starts_with("[Orchester] This conversation leads the module Webhooks v2"),
    "{accept}"
  );
  assert!(
    accept.contains("start_acceptance") && accept.contains("create_task"),
    "{accept}"
  );
  let deliver = standing_prompt(&event(Raised::Deliver));
  assert!(
    deliver.contains("deliver_module") && deliver.contains("is the person's"),
    "{deliver}"
  );
  let waits = standing_prompt(&event(Raised::DeliveryWaits {
    why: PersonsDelivery::NoAcceptance,
  }));
  assert!(
    waits.contains("names no acceptance pipeline") && waits.contains("deliver nothing"),
    "{waits}"
  );
}

#[test]
fn delivery_the_acceptance_brief_lists_the_tasks_their_criteria_and_what_was_left_unverified() {
  let tasks = [
    AcceptedTask {
      key: "KON-10".to_owned(),
      title: "Retry failed deliveries".to_owned(),
      criteria: vec!["A failed delivery is retried three times.".to_owned()],
      accepted_unverified: vec!["Verify".to_owned()],
    },
    AcceptedTask {
      key: "KON-11".to_owned(),
      title: "Signature header".to_owned(),
      criteria: Vec::new(),
      accepted_unverified: Vec::new(),
    },
  ];
  let brief = acceptance_brief("Webhooks v2", ("module/webhooks-v2", "main"), &tasks);
  for said in [
    "accepts the module Webhooks v2 as a whole",
    "git diff main...HEAD",
    "module/webhooks-v2",
    "- KON-10 — Retry failed deliveries",
    "A failed delivery is retried three times.",
    "Accepted unverified by automation, so verify it now: Verify.",
    "- KON-11 — Signature header",
  ] {
    assert!(brief.contains(said), "{said}: {brief}");
  }
  assert_eq!(acceptance_title("Webhooks v2"), "Accept the module Webhooks v2");
  assert_eq!(ACCEPTANCE_CRITERIA.len(), 3);
}
