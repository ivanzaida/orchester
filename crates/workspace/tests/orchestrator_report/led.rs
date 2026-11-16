//! OA-25: the report's section on what the Orchestrator did on its own and what it left to the person: none on a day
//! with nothing of it; otherwise the day's decisions with their reasons and how many are open, what is parked with
//! what the person is to do, the led targets that are stuck, and what each leading subscription spent.

use chrono::Days;
use orchester_core::{
  id::ChatId,
  orchestrator::{
    Ceilings, Chat, Subscription, WatchMode, WatchTarget,
    decision::{NewDecision, Reason, Review},
    parking::{Needs, Parking, Words},
  },
  time::Timestamp,
};

use super::{at, day, on, report, support::Fixture};

fn decided(chat: ChatId, reason: &str, when: Timestamp) -> NewDecision {
  NewDecision {
    chat,
    tool: "continue_run".to_owned(),
    task: Some("ORC-1".to_owned()),
    run: None,
    reason: Reason::new(reason).expect("a reason"),
    undo: None,
    at: when,
  }
}

#[test]
fn report_has_no_section_on_a_day_the_orchestrator_did_nothing_on_its_own() {
  let mut fixture = Fixture::new();
  fixture.everything(at(8));
  fixture.task("Add login", at(9));
  assert_eq!(
    report(&fixture, at(18)).led,
    None,
    "a subscription that only follows tells nothing here"
  );
}

#[test]
fn report_tells_what_the_orchestrator_decided_parked_and_is_stuck_on() {
  let mut fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Leading", at(8));
  fixture.db.keep_watch_entries(&chat, &[], at(8)).expect("conversation");
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let leading = Subscription::new(chat.id(), WatchTarget::Project, "", at(8)).in_mode(lead);
  fixture.db.start_subscription(&leading).expect("leads to the end");
  // A task without a pipeline is the person's to make ready: nothing can start, nothing moves.
  let task = fixture.task("No pipeline yet", at(9));
  let project = fixture.db.project().name().as_str().to_owned();

  let led = report(&fixture, at(10)).led.expect("a led target");
  assert_eq!(led.stuck, vec![project.clone()]);
  assert_eq!(led.leading.len(), 1, "{led:?}");
  let told = &led.leading[0];
  assert_eq!(
    (told.target.as_str(), told.standing.as_str()),
    (project.as_str(), "stuck")
  );
  assert_eq!((told.spent, told.budget), (0, None), "no budget was set");
  assert!(led.decisions.is_empty() && led.parked.is_empty(), "{led:?}");
  assert_eq!((led.left_to_person, &led.held), (0, &None));

  // A decision of the day and one of the day before: only the day's is told.
  let yesterday = day().checked_sub_days(Days::new(1)).expect("a day");
  fixture
    .db
    .keep_decision(&decided(chat.id(), "Yesterday's.", on(yesterday, 15, 0)))
    .expect("kept");
  let number = fixture
    .db
    .keep_decision(&decided(chat.id(), "The check is flaky.", at(11)))
    .expect("kept");
  let led = report(&fixture, at(12)).led.expect("a decision");
  assert_eq!(led.decisions.len(), 1, "{led:?}");
  let decision = &led.decisions[0];
  assert_eq!(
    (decision.number, decision.reason.as_str()),
    (number, "The check is flaky.")
  );
  assert_eq!(
    (decision.tool.as_str(), decision.review.as_str()),
    ("continue_run", "open")
  );
  assert_eq!(led.open_decisions, 1);
  assert!(
    fixture
      .db
      .review_decision(number, Review::Seen { at: at(13) })
      .expect("reviewed")
  );
  assert_eq!(report(&fixture, at(14)).led.expect("a decision").open_decisions, 0);

  let parking = Parking {
    task,
    run: None,
    needs: Needs::Decision,
    why: Words::new("The brief names no pipeline.").expect("words"),
    to_do: Words::new("Choose the task's pipeline.").expect("words"),
    by: chat.id(),
    at: at(15),
  };
  fixture.db.park_task(&parking).expect("parked");
  let led = report(&fixture, at(16)).led.expect("a parked task");
  assert_eq!(led.parked.len(), 1, "{led:?}");
  let parked = &led.parked[0];
  assert_eq!((parked.task.as_str(), parked.needs.as_str()), ("ORC-1", "decision"));
  assert_eq!(
    (parked.to_do.as_str(), parked.since),
    ("Choose the task's pipeline.", at(15))
  );
}
