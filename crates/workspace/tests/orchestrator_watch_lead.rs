//! Choosing to lead a target to the end (OA-10): the person's own /watch with *Lead to the end* chosen in its picker,
//! or whose order begins with those words, has its targets led, within the ceilings the project gives a new leading
//! subscription, with what follows the words as the order; a target the conversation only followed is led from then
//! on; the command keeps that it led; and the watch the agent applies at the person's request never chooses it. A
//! subscription that leads is only followed again, or led on within other ceilings, in place; the project's defaults
//! are kept per project.
//! `cargo test -p orchester-workspace --test orchestrator_watch_lead watch_lead`.

mod support;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Ceilings, Chat, ChatEvent, EntryKind, WatchMode, WatchTarget,
    watch::{NamedTarget, WatchCommand},
  },
  tokens::{TokenBudget, TokensUsed},
};
use orchester_workspace::orchestrator::{
  chat::ChatView,
  settings::{lead_defaults_in, save_lead_defaults_in},
  typed_ceilings::{TypedCeilings, Wrong},
  watch::{WatchError, follow_in, lead_in, set_ceilings_in, watch_asked_in, watch_in},
  watching::{LeadCounts, LedToEnd, WatchReads},
};
use support::{Fixture, at};

fn everything() -> NamedTarget {
  NamedTarget {
    target: WatchTarget::Project,
    name: None,
  }
}

/// How `chat` takes the whole project, and with which order.
fn taken(fixture: &Fixture, chat: ChatId) -> (WatchMode, Option<String>) {
  let subscriptions = fixture.db.chat_subscriptions(chat).expect("subscriptions");
  assert_eq!(subscriptions.len(), 1, "{subscriptions:?}");
  (subscriptions[0].mode(), subscriptions[0].order().map(str::to_owned))
}

fn leading() -> WatchMode {
  WatchMode::Lead {
    ceilings: Ceilings::default(),
  }
}

#[test]
fn watch_lead_is_chosen_by_the_first_words_of_the_persons_own_order() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Leading", at(10));
  let order = "Lead to the end: merge once the review passed.";
  watch_in(fixture.repo.path(), &chat, &[everything()], order).expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (leading(), Some("merge once the review passed.".to_owned())),
    "what follows the words is the order"
  );
  // The command is kept as the person wrote it.
  let kept = fixture.db.chat_entries(chat.id()).expect("entries");
  let command = kept.iter().find_map(|entry| match &entry.kind {
    EntryKind::Event(ChatEvent::WatchCommand {
      command: WatchCommand::Watch { order, .. },
    }) => Some(order.clone()),
    _ => None,
  });
  assert_eq!(command.as_deref(), Some(order));
  // Its line is told as one that leads to the end; a later watch that only follows is not.
  watch_in(fixture.repo.path(), &chat, &[everything()], "Ask before merging.").expect("watched");
  let view = ChatView::load_in(fixture.repo.path(), chat.id())
    .expect("read")
    .expect("kept");
  let lines: Vec<i64> = view
    .entries
    .iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Watch { .. })))
    .map(|entry| entry.seq)
    .collect();
  assert_eq!(lines.len(), 2);
  assert_eq!(view.led_to_end.get(&lines[0]), Some(&vec![WatchTarget::Project]));
  assert_eq!(view.led_to_end.get(&lines[1]), None);
}

#[test]
fn watch_lead_takes_over_a_target_the_conversation_only_followed() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Following", at(10));
  watch_in(fixture.repo.path(), &chat, &[everything()], "Ask before merging.").expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (WatchMode::Follow, Some("Ask before merging.".to_owned()))
  );
  // An order that only starts alike is an order.
  watch_in(fixture.repo.path(), &chat, &[everything()], "Lead to the endgame of Q4").expect("watched");
  assert_eq!(taken(&fixture, chat.id()).0, WatchMode::Follow);

  watch_in(fixture.repo.path(), &chat, &[everything()], "lead to the end").expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (leading(), None),
    "led from now on, by the default order"
  );
}

#[test]
fn watch_lead_is_never_chosen_by_the_watch_the_agent_applies() {
  let fixture = Fixture::new();
  let chat = Chat::named(ChatId::generate(), "Asked in words", at(10));
  fixture.db.chat(chat.id()).expect("read");
  watch_asked_in(
    fixture.repo.path(),
    &chat,
    &[everything()],
    "lead to the end: merge everything",
  )
  .expect("watched");
  assert_eq!(
    taken(&fixture, chat.id()),
    (WatchMode::Follow, Some("lead to the end: merge everything".to_owned())),
    "the agent's watch follows, and its order is kept whole"
  );
}

/// The commands kept in `chat`, each with whether it led its targets to the end.
fn commands(fixture: &Fixture, chat: ChatId) -> Vec<bool> {
  let kept = fixture.db.chat_entries(chat).expect("entries");
  kept
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Event(ChatEvent::WatchCommand {
        command: WatchCommand::Watch { to_end, .. },
      }) => Some(*to_end),
      _ => None,
    })
    .collect()
}

#[test]
fn watch_lead_defaults_are_the_projects_and_a_new_leading_subscription_takes_them() {
  let fixture = Fixture::new();
  let root = fixture.repo.path();
  assert_eq!(lead_defaults_in(root).expect("read"), Ceilings::default());
  // Out of bounds is no ceiling at all: there is nothing to save.
  assert!(Ceilings::new((6, 4), (1, 50), None).is_err());
  assert!(Ceilings::new((3, 11), (1, 50), None).is_err());
  assert!(Ceilings::new((3, 4), (4, 50), None).is_err());
  assert!(Ceilings::new((3, 4), (1, 101), None).is_err());
  assert_eq!(lead_defaults_in(root).expect("read"), Ceilings::default());

  let first = Chat::named(ChatId::generate(), "Before", at(10));
  lead_in(root, &first, &[everything()], "Merge what passes.").expect("led");
  assert_eq!(
    taken(&fixture, first.id()),
    (leading(), Some("Merge what passes.".to_owned())),
    "chosen in the picker: the built-in ceilings, the order whole"
  );

  let budget = TokenBudget::new(40_000_000).expect("a budget");
  let set = Ceilings::new((5, 2), (0, 25), Some(budget)).expect("ceilings");
  save_lead_defaults_in(root, &set).expect("saved");
  assert_eq!(lead_defaults_in(root).expect("read"), set);
  assert_eq!(
    taken(&fixture, first.id()).0,
    leading(),
    "a subscription already made keeps its own"
  );

  let task = NamedTarget {
    target: WatchTarget::WithoutModule,
    name: None,
  };
  let second = Chat::named(ChatId::generate(), "After", at(20));
  // The words in the order choose it too, and are not part of the order.
  lead_in(root, &second, std::slice::from_ref(&task), "lead to the end: ask first").expect("led");
  let led = fixture.db.chat_subscriptions(second.id()).expect("subscriptions");
  assert_eq!(led.len(), 1);
  assert_eq!(led[0].mode(), WatchMode::Lead { ceilings: set });
  assert_eq!(led[0].order(), Some("ask first"));
  assert_eq!(commands(&fixture, first.id()), [true], "the command keeps that it led");
  assert_eq!(commands(&fixture, second.id()), [true]);
}

#[test]
fn watch_lead_chosen_in_the_picker_is_told_on_its_line_and_a_following_command_is_not() {
  let fixture = Fixture::new();
  let root = fixture.repo.path();
  let chat = Chat::named(ChatId::generate(), "Leading", at(10));
  watch_in(root, &chat, &[everything()], "Ask before merging.").expect("followed");
  lead_in(root, &chat, &[everything()], "").expect("led from now on");
  assert_eq!(taken(&fixture, chat.id()), (leading(), None));
  assert_eq!(commands(&fixture, chat.id()), [false, true]);
  let view = ChatView::load_in(root, chat.id()).expect("read").expect("kept");
  let lines: Vec<i64> = view
    .entries
    .iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Watch { .. })))
    .map(|entry| entry.seq)
    .collect();
  assert_eq!(lines.len(), 2);
  assert_eq!(view.led_to_end.get(&lines[0]), None, "the following command's line");
  assert_eq!(view.led_to_end.get(&lines[1]), Some(&vec![WatchTarget::Project]));
}

#[test]
fn watch_lead_ceilings_and_mode_change_in_place_for_the_conversation_that_leads() {
  let fixture = Fixture::new();
  let root = fixture.repo.path();
  let (mine, theirs) = (
    Chat::named(ChatId::generate(), "Mine", at(10)),
    Chat::named(ChatId::generate(), "Theirs", at(11)),
  );
  let target = WatchTarget::Project;
  let ceilings = Ceilings::new((2, 0), (3, 100), None).expect("ceilings");
  assert!(matches!(
    set_ceilings_in(root, mine.id(), target, ceilings),
    Err(WatchError::NotWatched)
  ));
  assert!(matches!(
    follow_in(root, mine.id(), target),
    Err(WatchError::NotWatched)
  ));

  watch_in(root, &mine, &[everything()], "Ask before merging.").expect("followed");
  assert!(
    matches!(
      set_ceilings_in(root, mine.id(), target, ceilings),
      Err(WatchError::NotLedToEnd)
    ),
    "a target that is only followed has no ceilings"
  );
  assert_eq!(taken(&fixture, mine.id()).0, WatchMode::Follow);

  lead_in(root, &mine, &[everything()], "Ask before merging.").expect("led");
  let since = fixture.db.chat_subscriptions(mine.id()).expect("read")[0].since();
  assert!(matches!(
    set_ceilings_in(root, theirs.id(), target, ceilings),
    Err(WatchError::LedElsewhere { chat, .. }) if chat == mine.id()
  ));
  assert!(matches!(
    follow_in(root, theirs.id(), target),
    Err(WatchError::LedElsewhere { .. })
  ));
  assert_eq!(
    taken(&fixture, mine.id()).0,
    leading(),
    "another conversation changes nothing"
  );

  set_ceilings_in(root, mine.id(), target, ceilings).expect("changed");
  let now = fixture.db.chat_subscriptions(mine.id()).expect("read");
  assert_eq!(now.len(), 1);
  assert_eq!(now[0].mode(), WatchMode::Lead { ceilings });
  assert_eq!(
    now[0].since(),
    since,
    "the same subscription: what it spent goes on counting"
  );
  assert_eq!(now[0].order(), Some("Ask before merging."));
  assert_eq!(
    lead_defaults_in(root).expect("read"),
    Ceilings::default(),
    "this subscription only"
  );

  follow_in(root, mine.id(), target).expect("followed");
  let now = fixture.db.chat_subscriptions(mine.id()).expect("read");
  assert_eq!((now[0].mode(), now[0].since()), (WatchMode::Follow, since));
  follow_in(root, mine.id(), target).expect("one it follows already stays so");
}

#[test]
fn watch_lead_shows_in_the_watching_answer_with_its_ceilings_and_what_its_budget_spent() {
  let fixture = Fixture::new();
  let root = fixture.repo.path();
  let chat = Chat::named(ChatId::generate(), "Leading", at(10));
  let rest = NamedTarget {
    target: WatchTarget::WithoutModule,
    name: None,
  };
  lead_in(root, &chat, &[everything()], "").expect("led");
  watch_in(root, &chat, std::slice::from_ref(&rest), "").expect("followed");

  let watching = WatchReads::read(&fixture.db, root).expect("reads").watching(chat.id());
  let lead: Vec<Option<LedToEnd>> = watching.subscriptions.iter().map(|watched| watched.lead).collect();
  assert_eq!(
    lead,
    [
      Some(LedToEnd {
        ceilings: Ceilings::default(),
        spent: None
      }),
      None
    ],
    "no budget: nothing is read of what it spent"
  );
  assert_eq!(
    watching.follows,
    LeadCounts {
      without_module: true,
      ..LeadCounts::default()
    },
    "what it only follows"
  );

  let budget = TokenBudget::new(1_000).expect("a budget");
  let bounded = Ceilings::new((3, 4), (1, 50), Some(budget)).expect("ceilings");
  set_ceilings_in(root, chat.id(), WatchTarget::Project, bounded).expect("changed");
  let watching = WatchReads::read(&fixture.db, root).expect("reads").watching(chat.id());
  assert_eq!(
    watching.subscriptions[0].lead,
    Some(LedToEnd {
      ceilings: bounded,
      spent: Some(TokensUsed::ZERO)
    }),
    "with a budget, what its work spent since it leads"
  );
}
#[test]
fn watch_lead_says_which_run_a_task_that_was_run_again_is_on() {
  let mut fixture = Fixture::new();
  let once = fixture.task("Sign webhook payloads", at(4));
  fixture.started_run(once, at(5));
  let again = fixture.task("Retry failed deliveries", at(6));
  let mut first = fixture.started_run(again, at(7));
  fixture.succeed(&mut first, at(8));
  fixture.started_run(again, at(9));
  let root = fixture.repo.path().to_path_buf();
  let chat = Chat::named(ChatId::generate(), "Leading", at(10));

  watch_in(&root, &chat, &[everything()], "").expect("followed");
  let followed = WatchReads::read(&fixture.db, &root).expect("reads").watching(chat.id());
  assert!(
    followed.subscriptions[0].tasks.iter().all(|task| task.again.is_none()),
    "a task that is only followed has no ceiling to count against"
  );

  lead_in(&root, &chat, &[everything()], "").expect("led");
  let led = WatchReads::read(&fixture.db, &root).expect("reads").watching(chat.id());
  let said = |id| {
    let task = led.subscriptions[0]
      .tasks
      .iter()
      .find(|task| task.id == id)
      .expect("listed");
    task.again.clone()
  };
  assert_eq!(said(once), None, "its first run");
  let message = said(again).expect("run again");
  assert_eq!(message.key, "orchestrator.watch.task.again");
  let arg = |name: &str| {
    message
      .args
      .iter()
      .find(|(arg, _)| *arg == name)
      .map(|(_, value)| value.as_str().to_owned())
  };
  assert_eq!(
    (arg("run").as_deref(), arg("most").as_deref()),
    (Some("2"), Some("3")),
    "its second run of the three the ceiling allows"
  );
}

#[test]
fn watch_lead_typed_ceilings_are_read_field_by_field_and_out_of_bounds_is_refused() {
  let default = TypedCeilings::of(Ceilings::default(), ",");
  assert_eq!(
    (
      default.runs.as_str(),
      default.returns.as_str(),
      default.raises.as_str(),
      default.percent.as_str(),
      default.budget.as_str()
    ),
    ("3", "4", "1", "50", ""),
    "no budget is an empty field"
  );
  assert_eq!(default.read(","), Ok(Ceilings::default()));

  let budget = TokenBudget::new(40_000_000).expect("a budget");
  let most = Ceilings::new((5, 10), (3, 100), Some(budget)).expect("the most");
  let typed = TypedCeilings::of(most, ",");
  assert_eq!(typed.budget, "40,000,000", "an absolute number, grouped");
  assert_eq!(typed.read(","), Ok(most));
  let least = TypedCeilings {
    runs: " 1 ".to_owned(),
    returns: "0".to_owned(),
    raises: "0".to_owned(),
    percent: "1".to_owned(),
    budget: " ".to_owned(),
  };
  assert_eq!(
    least.read(","),
    Ok(Ceilings::new((1, 0), (0, 1), None).expect("the least"))
  );

  // The frame's out of bounds: 8 runs, by 150 %, a budget of "lots".
  let wrong = TypedCeilings {
    runs: "8".to_owned(),
    percent: "150".to_owned(),
    budget: "lots".to_owned(),
    ..default.clone()
  };
  assert_eq!(
    wrong.read(","),
    Err(vec![Wrong::Runs, Wrong::PercentAbove, Wrong::Budget])
  );
  for (field, value, why) in [
    ("runs", "0", Wrong::Runs),
    ("returns", "11", Wrong::Returns),
    ("raises", "4", Wrong::Raises),
    ("raises", "", Wrong::Raises),
    ("percent", "0", Wrong::PercentBelow),
    ("percent", "half", Wrong::PercentAbove),
    ("budget", "40M", Wrong::Budget),
    ("budget", "0", Wrong::Budget),
  ] {
    let mut typed = default.clone();
    let place = match field {
      "runs" => &mut typed.runs,
      "returns" => &mut typed.returns,
      "raises" => &mut typed.raises,
      "percent" => &mut typed.percent,
      _ => &mut typed.budget,
    };
    value.clone_into(place);
    assert_eq!(typed.read(","), Err(vec![why]), "{typed:?}");
  }
}
