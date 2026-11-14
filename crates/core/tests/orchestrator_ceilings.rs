//! The ceilings of work a conversation leads to the end (`orchestrator::watch`, OA-09): what they may be set to, the
//! person's defaults, what each allows, how they are kept, that a subscription kept before there was a choice follows,
//! and how a run's events count automation's returns and the raises of its limits.
//! `cargo test -p orchester-core --test orchestrator_ceilings ceilings`.

use std::num::NonZeroU64;

use orchester_core::{
  id::{ChatId, RunId},
  orchestrator::{
    Ceilings, Subscription, Subscriptions, WatchMode, WatchTarget,
    watch::{Ceiling, CeilingReached, CeilingsError, own_returns, raises},
  },
  resource::StepKey,
  run::{Answerer, EventSequence, RunEvent, RunEventKind, StuckDecision},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};

fn ceilings(runs: u8, returns: u8, raises: u8, percent: u8) -> Result<Ceilings, CeilingsError> {
  Ceilings::new((runs, returns), (raises, percent), None)
}

fn reached(ceiling: Ceiling, allowed: u64, used: u64) -> Result<(), CeilingReached> {
  Err(CeilingReached { ceiling, allowed, used })
}

#[test]
fn ceilings_are_the_persons_defaults_and_stay_within_their_bounds() {
  let default = Ceilings::default();
  assert_eq!(
    (
      default.runs_of_task(),
      default.returns_of_run(),
      default.raises_of_run(),
      default.raise_percent(),
      default.budget()
    ),
    (3, 4, 1, 50, None),
    "three runs of a task, four returns of a run, one raise by half, no budget"
  );
  assert_eq!(ceilings(3, 4, 1, 50), Ok(default));
  assert_eq!(ceilings(0, 4, 1, 50), Err(CeilingsError::Runs));
  assert_eq!(ceilings(6, 4, 1, 50), Err(CeilingsError::Runs));
  assert_eq!(ceilings(3, 11, 1, 50), Err(CeilingsError::Returns));
  assert_eq!(ceilings(3, 4, 4, 50), Err(CeilingsError::Raises));
  assert_eq!(ceilings(3, 4, 1, 0), Err(CeilingsError::RaisePercent));
  assert_eq!(ceilings(3, 4, 1, 101), Err(CeilingsError::RaisePercent));
  assert!(ceilings(1, 0, 0, 100).is_ok(), "the least and the most they may be");
  assert!(ceilings(5, 10, 3, 1).is_ok());
}

#[test]
fn ceilings_allow_runs_returns_and_raises_up_to_what_they_say() {
  let set = Ceilings::default();
  assert_eq!(set.another_run(2), Ok(()));
  assert_eq!(set.another_run(3), reached(Ceiling::RunsOfTask, 3, 3));
  assert_eq!(set.another_return(3), Ok(()));
  assert_eq!(set.another_return(4), reached(Ceiling::ReturnsOfRun, 4, 4));
  // One raise, by half at most; never to no limit at all.
  assert_eq!(set.raise(0, 1_000_000, Some(1_500_000)), Ok(()));
  assert_eq!(
    set.raise(0, 1_000_000, Some(1_500_001)),
    reached(Ceiling::RaiseBy, 1_500_000, 1_500_001)
  );
  assert_eq!(
    set.raise(0, 1_000_000, None),
    reached(Ceiling::RaiseBy, 1_500_000, u64::MAX)
  );
  assert_eq!(
    set.raise(1, 1_000_000, Some(1_100_000)),
    reached(Ceiling::RaisesOfRun, 1, 1)
  );
  let none = ceilings(1, 0, 0, 50).expect("ceilings");
  assert_eq!(none.another_return(0), reached(Ceiling::ReturnsOfRun, 0, 0));
  assert_eq!(none.raise(0, 100, Some(110)), reached(Ceiling::RaisesOfRun, 0, 0));
}

#[test]
fn ceilings_budget_stops_new_work_once_it_is_spent() {
  let budget = TokenBudget::new(1_000).expect("above zero");
  let set = Ceilings::new((3, 4), (1, 50), Some(budget)).expect("ceilings");
  assert_eq!(set.within_budget(TokensUsed::new(999)), Ok(()));
  assert_eq!(
    set.within_budget(TokensUsed::new(1_000)),
    reached(Ceiling::Budget, 1_000, 1_000)
  );
  assert_eq!(
    Ceilings::default().within_budget(TokensUsed::new(u64::MAX)),
    Ok(()),
    "no budget, no ceiling"
  );
}

#[test]
fn ceilings_are_kept_with_a_leading_subscription_and_one_kept_before_follows() {
  let leading = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let kept = serde_json::to_string(&leading).expect("written");
  assert_eq!(serde_json::from_str::<WatchMode>(&kept).expect("read"), leading);
  assert_eq!(leading.ceilings(), Some(Ceilings::default()));
  assert_eq!(WatchMode::Follow.ceilings(), None);
  let outside =
    r#"{"mode":"lead","ceilings":{"runs_of_task":9,"returns_of_run":4,"raises_of_run":1,"raise_percent":50}}"#;
  assert!(
    serde_json::from_str::<WatchMode>(outside).is_err(),
    "ceilings outside their bounds are not read"
  );

  let subscription = Subscription::new(ChatId::generate(), WatchTarget::Project, "", Timestamp::now());
  assert_eq!(subscription.mode(), WatchMode::Follow, "a new subscription follows");
  let mut before = serde_json::to_value(&subscription).expect("written");
  before.as_object_mut().expect("an object").remove("mode");
  let read: Subscription = serde_json::from_value(before).expect("read");
  assert_eq!(
    read.mode(),
    WatchMode::Follow,
    "one kept before there was a choice follows"
  );
  assert_eq!(subscription.in_mode(leading).mode(), leading);
}

#[test]
fn ceilings_stay_with_a_target_that_is_taken_over_or_renewed() {
  let (theirs, mine) = (ChatId::generate(), ChatId::generate());
  let leading = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let led = Subscription::new(theirs, WatchTarget::Project, "to the end", Timestamp::now()).in_mode(leading);
  let set = Subscriptions::new([led]).expect("one leader");
  let taking = Subscription::new(mine, WatchTarget::Project, "mine now", Timestamp::now());
  let taken = set.take_over(taking.clone(), None).into_set();
  let now = taken.of(WatchTarget::Project).expect("led");
  assert_eq!((now.chat(), now.mode()), (mine, leading), "still led to the end");
  assert_eq!(
    taking.clone().succeeding(None).mode(),
    WatchMode::Follow,
    "nobody led it"
  );
  let free = Subscriptions::default().take_over(taking, None).into_set();
  assert_eq!(
    free.of(WatchTarget::Project).map(Subscription::mode),
    Some(WatchMode::Follow)
  );
}

fn event(run: RunId, sequence: u64, kind: RunEventKind) -> RunEvent {
  let sequence = EventSequence::new(NonZeroU64::new(sequence).expect("above zero"));
  RunEvent::new(run, sequence, Timestamp::now(), kind)
}

#[test]
fn ceilings_count_automations_returns_and_every_raise_of_a_run() {
  let run = RunId::generate();
  let step = || StepKey::new("implement").expect("a step key");
  let decided = |decision, by| RunEventKind::StuckDecided { decision, by };
  let events = [
    event(run, 1, decided(StuckDecision::Retry, Answerer::Automation)),
    event(run, 2, decided(StuckDecision::Retry, Answerer::Person)),
    event(run, 3, decided(StuckDecision::Stop, Answerer::Automation)),
    event(
      run,
      4,
      RunEventKind::ReturnDecided {
        step: step(),
        by: Answerer::Automation,
      },
    ),
    event(
      run,
      5,
      RunEventKind::RecoveryDecided {
        step: step(),
        by: Answerer::Person,
      },
    ),
    event(
      run,
      6,
      RunEventKind::LimitRaised {
        budget: None,
        max_duration: None,
      },
    ),
    event(
      run,
      7,
      RunEventKind::StepBudgetRaised {
        step: step(),
        budget: None,
      },
    ),
    event(run, 8, RunEventKind::Resumed),
  ];
  assert_eq!(
    own_returns(&events),
    2,
    "automation's retry and its return of the ended run; a person's are theirs, and a stop is no return"
  );
  assert_eq!(raises(&events), 2, "every raise, whoever made it");
}

#[test]
fn ceilings_leading_is_asked_for_by_the_first_words_of_an_order() {
  use orchester_core::orchestrator::watch::asked_to_lead;
  assert_eq!(asked_to_lead("lead to the end"), Some(""));
  assert_eq!(
    asked_to_lead("  Lead To The End: merge when green"),
    Some("merge when green")
  );
  assert_eq!(
    asked_to_lead("lead to the end. Ask before a push."),
    Some("Ask before a push.")
  );
  assert_eq!(asked_to_lead("lead to the endgame"), None, "another word");
  assert_eq!(asked_to_lead("Please lead to the end"), None, "only at its start");
  assert_eq!(asked_to_lead(""), None);
  assert_eq!(asked_to_lead("веди до кінця"), None, "the words are these words");
}
