//! Choosing *Follow* or *Lead to the end* in a /watch picker (`L03 · Choosing · Follow or Lead to the end` `xE361`,
//! board `Orchestrator autonomy · frames to approve`; OA-10): the row over the picker's keys with its two ways and
//! what the chosen one means, Tab and a press switching it, the composer's badge naming it, and the command applied
//! as chosen: the conversation leads the targets to the end within the project's ceilings, the command keeps that it
//! did, its bubble says so and its line reads *Leading … to the end*. Follow is where a command starts; choosing is
//! a person's. `cargo test -p orchester-app --features test-support --test orchestrator watch_lead`.

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  id::{ChatId, ModuleId},
  orchestrator::{Ceilings, ChatEvent, EntryKind, WatchMode, WatchTarget, watch::WatchCommand},
  store::TaskStore,
};

use super::{
  harness::Orchestrator,
  watch_module::{BADGE, Kontur, PICKER, badge_words, focus_field, kontur_with, open_picker, row},
};

const MODE: &str = "orchestrator-composer-watch-module-mode";
const FOLLOWS: &str = "Tells you and answers agents; you start, merge and deliver.";
const LEADS: &str = "Starts its tasks, runs failed ones again and leaves you what it cannot do.";

/// What the row says the chosen way means.
fn meaning(screen: &mut Orchestrator, mode: &str) -> String {
  screen.text(&format!("{mode}-meaning"))
}

/// How conversation `chat` takes `module`; `None` while it does not watch it.
fn taken(screen: &Orchestrator, chat: ChatId, module: ModuleId) -> Option<WatchMode> {
  let subscriptions = screen.db().active_subscriptions().expect("read");
  subscriptions
    .of(WatchTarget::Module(module))
    .filter(|led| led.chat() == chat)
    .map(|led| led.mode())
}

/// Whether each /watch command kept in `chat` led its targets to the end, oldest first.
fn commands(screen: &Orchestrator, chat: ChatId) -> Vec<bool> {
  let entries = screen.db().chat_entries(chat).expect("entries");
  entries
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
fn orchestrator_watch_lead_is_chosen_with_tab_or_a_press_and_the_badge_names_it() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur_with(|_| {});
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  let shown = screen.text(PICKER).replace('\u{a0}', " ");
  for words in [
    "What it does with them",
    "Follow",
    "Lead to the end",
    FOLLOWS,
    "Tab",
    "Follow or lead",
    "Pick and close",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  let order: Vec<usize> = ["Everything in", "What it does with them", "Pick and close"]
    .iter()
    .map(|words| shown.find(words).expect("shown"))
    .collect();
  assert!(
    order.windows(2).all(|pair| pair[0] < pair[1]),
    "the way sits between the rows and the keys: {order:?}"
  );
  assert_eq!(badge_words(&mut screen, BADGE), "Watch", "Follow is where it starts");

  // Tab switches the way, wherever the focus is in the picker; the badge names it.
  screen.key(("Tab", "Tab"), false);
  screen.until("Lead to the end", |screen| meaning(screen, MODE) == LEADS);
  assert_eq!(badge_words(&mut screen, BADGE), "Lead to the end");
  screen.key(("Tab", "Tab"), false);
  screen.until("Follow again", |screen| meaning(screen, MODE) == FOLLOWS);
  assert_eq!(badge_words(&mut screen, BADGE), "Watch");

  // A press on a way chooses it.
  screen.click(&format!("{MODE}-lead"));
  screen.until("Lead to the end", |screen| meaning(screen, MODE) == LEADS);
  screen.click(&format!("{MODE}-lead"));
  assert_eq!(meaning(&mut screen, MODE), LEADS, "the chosen way stays chosen");
  screen.click(&format!("{MODE}-follow"));
  screen.until("Follow", |screen| meaning(screen, MODE) == FOLLOWS);
  screen.click(&format!("{MODE}-lead"));
  screen.until("Lead to the end", |screen| meaning(screen, MODE) == LEADS);

  // Automation choosing it is refused, and the way stays.
  screen.click(&format!("{MODE}-follow"));
  screen.until("Follow", |screen| meaning(screen, MODE) == FOLLOWS);
  with_synthetic_input(|| screen.key(("Tab", "Tab"), false));
  assert!(screen.window.refused_press().is_some(), "refused to the client");
  with_synthetic_input(|| screen.click(&format!("{MODE}-lead")));
  assert_eq!(meaning(&mut screen, MODE), FOLLOWS, "synthetic input chooses nothing");
  assert_eq!(badge_words(&mut screen, BADGE), "Watch");

  // Chosen by the person, picked and sent: the conversation leads the module to the end.
  screen.key(("Tab", "Tab"), false);
  screen.until("Lead to the end", |screen| meaning(screen, MODE) == LEADS);
  screen.key(("Enter", "Enter"), false);
  screen.until("Webhooks v2 in the badge, the picker closed", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == "Lead to the end · Webhooks v2"
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| taken(screen, here, webhooks).is_some());
  assert_eq!(
    taken(&screen, here, webhooks),
    Some(WatchMode::Lead {
      ceilings: Ceilings::default()
    }),
    "within the project's ceilings"
  );
  assert_eq!(commands(&screen, here), [true], "the command keeps that it led");
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
  screen.until("its bubble and its line", |screen| {
    let said = screen.text("orchestrator-conversation");
    said.contains("/watch-module") && said.contains("Leading Webhooks v2 to the end")
  });
  let entries = screen.db().chat_entries(here).expect("entries");
  let command = entries
    .iter()
    .find(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::WatchCommand { .. })))
    .expect("the command");
  let bubble = format!("orchestrator-watch-wait-{}", command.seq);
  assert_eq!(
    screen.text(&format!("{bubble}-lead")),
    "Lead to the end",
    "the bubble's chip: {}",
    screen.text(&bubble)
  );

  // The next command starts at Follow again.
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  assert_eq!(meaning(&mut screen, MODE), FOLLOWS);
  assert_eq!(badge_words(&mut screen, BADGE), "Watch");
}

#[test]
fn orchestrator_watch_lead_following_stays_the_default_and_a_followed_module_is_led_by_the_next_command() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur_with(|_| {});
  // Picked with nothing chosen, the command follows, as it always did.
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.key(("Enter", "Enter"), false);
  screen.until("the badge", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == "Watch · Webhooks v2"
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| {
    taken(screen, here, webhooks) == Some(WatchMode::Follow)
  });
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
  assert_eq!(commands(&screen, here), [false]);

  // The same module, chosen again with Lead to the end: the conversation leads it from now on.
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.key(("Tab", "Tab"), false);
  screen.until("Lead to the end", |screen| meaning(screen, MODE) == LEADS);
  screen.key(("Enter", "Enter"), false);
  screen.until("the badge", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == "Lead to the end · Webhooks v2"
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("led to the end", |screen| {
    matches!(taken(screen, here, webhooks), Some(WatchMode::Lead { .. }))
  });
  assert_eq!(commands(&screen, here), [false, true]);
}

#[test]
fn orchestrator_watch_lead_is_chosen_in_the_task_picker_too() {
  let Kontur {
    mut screen,
    here,
    limiter,
    ..
  } = kontur_with(|_| {});
  focus_field(&mut screen);
  screen.set("orchestrator-input", "/watch-task");
  let command = "orchestrator-composer-slash-watch-task";
  screen.until("Watch tasks focused", |screen| {
    screen.exists(command) && screen.text(command).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  let (picker, badge, mode) = (
    "orchestrator-composer-watch-task-picker",
    "orchestrator-composer-badge-watch-task",
    "orchestrator-composer-watch-task-mode",
  );
  screen.until("the task picker", |screen| screen.exists(picker) && screen.exists(mode));
  let shown = screen.text(picker);
  for words in ["What it does with them", "Follow or lead", FOLLOWS] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  screen.key(("Tab", "Tab"), false);
  screen.until("Lead to the end", |screen| meaning(screen, mode) == LEADS);
  assert_eq!(badge_words(&mut screen, badge), "Lead to the end");

  screen.set("orchestrator-composer-watch-task-search", "rate lim");
  let key = screen
    .db()
    .task(limiter)
    .expect("read")
    .expect("the task")
    .key()
    .to_string();
  let limiter_row = format!("orchestrator-composer-watch-task-{key}");
  screen.until("only the limiter", |screen| screen.exists(&limiter_row));
  assert_eq!(
    meaning(&mut screen, mode),
    LEADS,
    "the way stays while the search filters"
  );
  screen.key(("Enter", "Enter"), false);
  let words = format!("Lead to the end · {key}");
  screen.until("the task badge, the picker closed", |screen| {
    !screen.exists(picker) && badge_words(screen, badge) == words
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the task led to the end", |screen| {
    let subscriptions = screen.db().active_subscriptions().expect("read");
    subscriptions
      .of(WatchTarget::Task(limiter))
      .is_some_and(|led| led.chat() == here && led.mode().ceilings().is_some())
  });
}
